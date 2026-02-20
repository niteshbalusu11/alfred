import SwiftUI

struct HomeView: View {
    @ObservedObject var model: AppModel
    @StateObject private var transcriptionController = VoiceTranscriptionController()
    @State private var responseSpeaker = AssistantResponseSpeaker()
    @State private var composerText = ""
    @FocusState private var isComposerFocused: Bool

    private var statusBadge: (title: String, style: AppStatusBadge.Style) {
        switch transcriptionController.status {
        case .idle:
            return ("Ready", .neutral)
        case .requestingPermissions:
            return ("Checking access", .warning)
        case .listening:
            return ("Listening", .success)
        case .permissionDenied:
            return ("Permission needed", .danger)
        case .restricted:
            return ("Restricted", .danger)
        case .unavailable:
            return ("Unavailable", .danger)
        case .failed:
            return ("Error", .danger)
        }
    }

    private var statusMessage: String {
        switch transcriptionController.status {
        case .idle:
            return "Type a message or tap the mic to speak."
        case .requestingPermissions:
            return "Requesting microphone and speech access."
        case .listening:
            return "Listening on-device. Transcript updates live."
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

    private var liveDraftText: String {
        transcriptionController.isListening ? transcriptionController.transcript : ""
    }

    private var canSendMessage: Bool {
        !composerText.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
            && !model.isLoading(.queryAssistant)
    }

    var body: some View {
        VStack(spacing: 12) {
            statusHeader
                .padding(.horizontal, AppTheme.Layout.screenPadding)
                .padding(.top, 8)

            AssistantConversationView(
                messages: model.assistantConversation,
                draftMessage: liveDraftText,
                isLoading: model.isLoading(.queryAssistant),
                showsHeader: false,
                emptyStateText: "Start a chat with Alfred. You can type or use the mic."
            )
            .frame(maxWidth: .infinity, maxHeight: .infinity)
            .padding(.horizontal, AppTheme.Layout.screenPadding)
            .padding(.bottom, 8)
        }
        .safeAreaInset(edge: .bottom, spacing: 0) {
            composerBar
        }
        .appScreenBackground()
        .onDisappear {
            transcriptionController.stopRecording()
            responseSpeaker.stop()
        }
        .onChange(of: transcriptionController.transcript) { _, newValue in
            guard transcriptionController.isListening else { return }
            composerText = newValue
        }
        .onChange(of: model.assistantResponseText) { _, newValue in
            responseSpeaker.speak(newValue)
        }
    }

    private var statusHeader: some View {
        HStack(spacing: 10) {
            AppStatusBadge(title: statusBadge.title, style: statusBadge.style)

            ListeningDotsIndicator(isActive: transcriptionController.isListening)
                .frame(width: 28, height: 12)

            Text(statusMessage)
                .font(.footnote.weight(.semibold))
                .foregroundStyle(AppTheme.Colors.textSecondary)
                .lineLimit(2)

            Spacer(minLength: 0)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }

    private var composerBar: some View {
        VStack(spacing: 10) {
            HStack(alignment: .bottom, spacing: 10) {
                Button {
                    Task { await toggleRecording() }
                } label: {
                    Image(systemName: transcriptionController.isListening ? "stop.fill" : "mic.fill")
                        .font(.system(size: 18, weight: .black))
                        .foregroundStyle(AppTheme.Colors.ink)
                        .frame(width: 44, height: 44)
                        .background(
                            Circle()
                                .fill(AppTheme.Colors.paper.opacity(
                                    transcriptionController.isRequestingPermissions ? 0.5 : 1
                                ))
                        )
                        .overlay(
                            Circle()
                                .stroke(AppTheme.Colors.ink, lineWidth: AppTheme.Layout.cartoonStrokeWidth)
                        )
                }
                .buttonStyle(.plain)
                .disabled(transcriptionController.isRequestingPermissions)
                .accessibilityLabel(transcriptionController.isListening ? "Stop recording" : "Start recording")

                TextField("Message Alfred…", text: $composerText, axis: .vertical)
                    .lineLimit(1...4)
                    .focused($isComposerFocused)
                    .submitLabel(.send)
                    .onSubmit {
                        sendMessage()
                    }
                    .padding(.horizontal, 12)
                    .padding(.vertical, 11)
                    .foregroundStyle(AppTheme.Colors.textPrimary)
                    .background(AppTheme.Colors.surfaceElevated)
                    .clipShape(RoundedRectangle(cornerRadius: 14, style: .continuous))
                    .overlay(
                        RoundedRectangle(cornerRadius: 14, style: .continuous)
                            .stroke(AppTheme.Colors.outline.opacity(0.8), lineWidth: 1)
                    )

                Button {
                    sendMessage()
                } label: {
                    Image(systemName: model.isLoading(.queryAssistant) ? "hourglass" : "arrow.up")
                        .font(.system(size: 18, weight: .black))
                        .foregroundStyle(AppTheme.Colors.ink)
                        .frame(width: 44, height: 44)
                        .background(
                            Circle()
                                .fill(AppTheme.Colors.paper.opacity(canSendMessage ? 1 : 0.42))
                        )
                        .overlay(
                            Circle()
                                .stroke(AppTheme.Colors.ink, lineWidth: AppTheme.Layout.cartoonStrokeWidth)
                        )
                }
                .buttonStyle(.plain)
                .disabled(!canSendMessage)
                .accessibilityLabel("Send message")
            }

            HStack(spacing: 10) {
                Button("Clear Chat") {
                    clearChat()
                }
                .font(.caption.weight(.bold))
                .foregroundStyle(AppTheme.Colors.textSecondary)
                .buttonStyle(.plain)
                .disabled(
                    model.assistantConversation.isEmpty
                        && composerText.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
                )

                Spacer(minLength: 0)

                if transcriptionController.isListening {
                    Text("Listening…")
                        .font(.caption.weight(.bold))
                        .foregroundStyle(AppTheme.Colors.textSecondary)
                }
            }
        }
        .padding(.horizontal, AppTheme.Layout.screenPadding)
        .padding(.top, 10)
        .padding(.bottom, 12)
        .background(AppTheme.Colors.background.opacity(0.98))
        .overlay(alignment: .top) {
            Rectangle()
                .fill(AppTheme.Colors.outline.opacity(0.25))
                .frame(height: 1)
        }
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
        transcriptionController.stopRecording()
        composerText = ""
        Task {
            await model.queryAssistant(query: query)
        }
    }

    private func clearChat() {
        transcriptionController.stopRecording()
        transcriptionController.clearTranscript()
        composerText = ""
        model.clearAssistantConversation()
    }
}

private struct ListeningDotsIndicator: View {
    let isActive: Bool

    var body: some View {
        TimelineView(.animation(minimumInterval: 1.0 / 24.0, paused: !isActive)) { timeline in
            let time = timeline.date.timeIntervalSinceReferenceDate

            HStack(spacing: 4) {
                ForEach(0..<3, id: \.self) { index in
                    let phase = time * 5.1 + (Double(index) * 0.6)
                    let progress = (sin(phase) + 1) * 0.5
                    let height = isActive ? (5 + (progress * 5)) : 4.0
                    let opacity = isActive ? (0.35 + (progress * 0.65)) : 0.3

                    Capsule(style: .continuous)
                        .fill(AppTheme.Colors.paper.opacity(opacity))
                        .frame(width: 4, height: height)
                }
            }
        }
        .accessibilityHidden(true)
    }
}

#Preview {
    HomeView(model: AppModel())
}
