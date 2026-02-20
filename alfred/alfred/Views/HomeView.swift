import SwiftUI

struct HomeView: View {
    @ObservedObject var model: AppModel
    @StateObject private var transcriptionController = VoiceTranscriptionController()
    @State private var responseSpeaker = AssistantResponseSpeaker()
    @State private var composerText = ""
    @State private var lastSpokenAssistantMessageID: UUID?
    @FocusState private var isComposerFocused: Bool
    private let topActionButtonScale: CGFloat = 1.17

    private var hasTypedMessage: Bool {
        !composerText.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
    }

    private var canSendMessage: Bool {
        hasTypedMessage && !model.isLoading(.queryAssistant)
    }

    private var voiceStatusText: String? {
        switch transcriptionController.status {
        case .idle:
            return nil
        case .listening:
            return "Listening on-device..."
        case .requestingPermissions:
            return "Requesting microphone and speech access..."
        case .permissionDenied:
            return "Enable Microphone and Speech Recognition in iOS Settings."
        case .restricted:
            return "Speech recognition is restricted on this device."
        case .unavailable:
            return "Speech recognition is unavailable for this locale."
        case .failed(let message):
            return message
        }
    }

    var body: some View {
        VStack(spacing: 0) {
            topBar
                .padding(.horizontal, 10)
                .padding(.top, 8)
                .padding(.bottom, 6)

            AssistantConversationView(
                messages: model.assistantConversation,
                draftMessage: "",
                isLoading: model.isLoading(.queryAssistant),
                showsHeader: false,
                emptyStateText: "Ask Anything"
            )
            .padding(.horizontal, 12)
            .padding(.top, 8)
            .padding(.bottom, 6)
            .contentShape(Rectangle())
            .onTapGesture {
                isComposerFocused = false
            }
        }
        .safeAreaInset(edge: .bottom, spacing: 0) {
            inputDock
        }
        .appScreenBackground()
        .ignoresSafeArea(.keyboard, edges: .bottom)
        .toolbar {
            ToolbarItemGroup(placement: .keyboard) {
                Spacer(minLength: 0)
                Button("Done") {
                    isComposerFocused = false
                }
            }
        }
        .onDisappear {
            transcriptionController.stopRecording()
            responseSpeaker.stop()
        }
        .onChange(of: transcriptionController.transcript) { _, newValue in
            guard transcriptionController.isListening else { return }
            composerText = newValue
        }
    }

    private var topBar: some View {
        HStack(spacing: 10) {
            circleIconButton(systemName: "text.bubble", scale: topActionButtonScale) {
                openThreadsScreen()
            }

            Spacer(minLength: 0)
            circleIconButton(systemName: "square.and.pencil", scale: topActionButtonScale) {
                clearChat()
            }
        }
    }

    private var inputDock: some View {
        VStack(spacing: 8) {
            composerContainer

            if let voiceStatusText {
                Text(voiceStatusText)
                    .font(.caption.weight(.semibold))
                    .foregroundStyle(AppTheme.Colors.textSecondary)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .padding(.horizontal, 6)
            }
        }
        .padding(.horizontal, 10)
        .padding(.top, 6)
        .padding(.bottom, 10)
        .background(
            LinearGradient(
                colors: [
                    AppTheme.Colors.background.opacity(0.0),
                    AppTheme.Colors.background.opacity(0.86),
                    AppTheme.Colors.background.opacity(0.98),
                ],
                startPoint: .top,
                endPoint: .bottom
            )
        )
    }

    @ViewBuilder
    private var composerContainer: some View {
        if #available(iOS 26, *) {
            composerContent
                .padding(10)
                .glassEffect(.regular.tint(AppTheme.Colors.paper.opacity(0.05)).interactive(), in: .rect(cornerRadius: 22))
                .contentShape(RoundedRectangle(cornerRadius: 22, style: .continuous))
                .onTapGesture {
                    isComposerFocused = true
                }
        } else {
            composerContent
                .padding(10)
                .background(.ultraThinMaterial, in: RoundedRectangle(cornerRadius: 22, style: .continuous))
                .overlay(
                    RoundedRectangle(cornerRadius: 22, style: .continuous)
                        .stroke(AppTheme.Colors.paper.opacity(0.12), lineWidth: 1)
                )
                .contentShape(RoundedRectangle(cornerRadius: 22, style: .continuous))
                .onTapGesture {
                    isComposerFocused = true
                }
        }
    }

    private var composerContent: some View {
        VStack(spacing: 8) {
            TextField("Ask Anything", text: $composerText, axis: .vertical)
                .lineLimit(1...4)
                .focused($isComposerFocused)
                .submitLabel(.send)
                .onSubmit {
                    sendMessage()
                }
                .font(.system(size: 17, weight: .regular))
                .foregroundStyle(AppTheme.Colors.textPrimary)
                .frame(maxWidth: .infinity, alignment: .leading)
                .padding(.horizontal, 8)
                .padding(.vertical, 6)

            HStack(spacing: 8) {
                Button {
                    Task { await toggleRecording() }
                } label: {
                    Image(systemName: transcriptionController.isListening ? "stop.fill" : "mic.fill")
                        .font(.system(size: 13, weight: .bold))
                        .foregroundStyle(AppTheme.Colors.textPrimary)
                        .frame(width: 30, height: 30)
                        .background(AppTheme.Colors.surface.opacity(0.7), in: Circle())
                }
                .buttonStyle(.plain)
                .disabled(transcriptionController.isRequestingPermissions || model.isLoading(.queryAssistant))

                Spacer(minLength: 0)

                trailingActionButton
            }
        }
    }

    @ViewBuilder
    private var trailingActionButton: some View {
        if hasTypedMessage {
            Button {
                sendMessage()
            } label: {
                Image(systemName: model.isLoading(.queryAssistant) ? "hourglass" : "arrow.up")
                    .font(.system(size: 14, weight: .black))
                    .foregroundStyle(AppTheme.Colors.ink)
                    .frame(width: 34, height: 34)
                    .background(AppTheme.Colors.paper.opacity(canSendMessage ? 1 : 0.35), in: Circle())
            }
            .buttonStyle(.plain)
            .disabled(!canSendMessage)
        } else {
            Button {
                Task { await toggleRecording() }
            } label: {
                Text(transcriptionController.isListening ? "Stop" : "Speak")
                    .font(.subheadline.weight(.bold))
                    .foregroundStyle(AppTheme.Colors.ink)
                    .padding(.horizontal, 14)
                    .padding(.vertical, 8)
                    .background(AppTheme.Colors.paper, in: Capsule(style: .continuous))
            }
            .buttonStyle(.plain)
            .disabled(transcriptionController.isRequestingPermissions || model.isLoading(.queryAssistant))
        }
    }

    private func circleIconButton(
        systemName: String,
        scale: CGFloat = 1.0,
        action: @escaping () -> Void = {}
    ) -> some View {
        Button(action: action) {
            Image(systemName: systemName)
                .font(.system(size: 15 * scale, weight: .bold))
                .foregroundStyle(AppTheme.Colors.textPrimary)
                .frame(width: 40 * scale, height: 40 * scale)
                .background(AppTheme.Colors.surface.opacity(0.65), in: Circle())
        }
        .buttonStyle(.plain)
    }

    private func toggleRecording() async {
        if transcriptionController.isListening {
            transcriptionController.stopRecording()
            return
        }

        responseSpeaker.stop()
        isComposerFocused = false
        await transcriptionController.startRecording()
    }

    private func sendMessage() {
        let query = composerText.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !query.isEmpty else { return }

        let previousAssistantMessageID = model.assistantConversation.last(where: { $0.role == .assistant })?.id
        transcriptionController.stopRecording()
        composerText = ""
        isComposerFocused = false

        Task { @MainActor in
            await model.queryAssistant(query: query)

            guard let latestAssistantMessage = model.assistantConversation.last(where: { $0.role == .assistant }) else {
                return
            }
            guard latestAssistantMessage.id != previousAssistantMessageID else {
                return
            }
            guard latestAssistantMessage.id != lastSpokenAssistantMessageID else {
                return
            }

            lastSpokenAssistantMessageID = latestAssistantMessage.id
            responseSpeaker.speak(latestAssistantMessage.text)
        }
    }

    private func clearChat() {
        transcriptionController.stopRecording()
        transcriptionController.clearTranscript()
        composerText = ""
        isComposerFocused = false
        model.clearAssistantConversation()
    }

    private func openThreadsScreen() {
        isComposerFocused = false
        withAnimation(.spring(response: 0.24, dampingFraction: 0.92)) {
            model.selectedTab = .threads
        }
    }

}

#Preview {
    HomeView(model: AppModel())
}
