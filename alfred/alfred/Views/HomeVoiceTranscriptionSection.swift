import ClerkKit
import SwiftUI

struct HomeVoiceTranscriptionSection: View {
    @ObservedObject var model: AppModel
    @StateObject private var transcriptionController = VoiceTranscriptionController()
    @State private var responseSpeaker = AssistantResponseSpeaker()

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
            return "Tap the mic and start talking. Transcription stays on-device."
        case .requestingPermissions:
            return "Waiting for speech and microphone authorization."
        case .listening:
            return "Listening now. Your words appear in real time."
        case .permissionDenied:
            return "Enable Microphone and Speech Recognition in iOS Settings."
        case .restricted:
            return "Speech recognition is restricted on this device."
        case .unavailable:
            return "Speech recognition is not available for your current locale."
        case .failed(let message):
            return message
        }
    }

    var body: some View {
        VStack(spacing: 18) {
            HStack {
                AppStatusBadge(title: statusBadge.title, style: statusBadge.style)
            }
            .frame(maxWidth: .infinity, alignment: .center)

            Text("Voice Input")
                .font(.system(size: 30, weight: .black, design: .rounded))
                .foregroundStyle(AppTheme.Colors.textPrimary)
                .frame(maxWidth: .infinity, alignment: .center)

            waveformView

            Text(statusMessage)
                .font(.footnote)
                .foregroundStyle(AppTheme.Colors.textSecondary)
                .multilineTextAlignment(.center)
                .frame(maxWidth: .infinity, alignment: .center)
                .padding(.horizontal, 24)
                .fixedSize(horizontal: false, vertical: true)

            transcriptView
            assistantResponseView
            controlButtons
        }
        .frame(maxWidth: .infinity, alignment: .center)
        .onDisappear {
            transcriptionController.stopRecording()
            responseSpeaker.stop()
        }
        .onChange(of: model.assistantResponseText) { oldValue, newValue in
            responseSpeaker.speak(newValue)
        }
    }

    private var waveformView: some View {
        ZStack {
            Ellipse()
                .fill(AppTheme.Colors.surfaceElevated.opacity(0.44))
                .frame(height: 56)
                .blur(radius: 12)

            LiveWaveformView(isActive: transcriptionController.isListening)
                .frame(height: 116)
        }
        .frame(maxWidth: .infinity)
        .clipped()
    }

    private var transcriptView: some View {
        Group {
            if transcriptionController.transcript.isEmpty {
                Text("Transcript appears here while you speak.")
                    .foregroundStyle(AppTheme.Colors.textSecondary)
                    .frame(maxWidth: .infinity, alignment: .center)
            } else {
                ScrollView(.vertical, showsIndicators: false) {
                    Text(transcriptionController.transcript)
                        .multilineTextAlignment(.center)
                        .frame(maxWidth: .infinity, alignment: .center)
                }
            }
        }
        .font(.subheadline.weight(.semibold))
        .multilineTextAlignment(.center)
        .frame(maxWidth: .infinity, minHeight: 96, maxHeight: 170, alignment: .top)
        .padding(.horizontal, 12)
        .shadow(color: AppTheme.Colors.shadow.opacity(0.35), radius: 0, x: 0, y: 2)
    }

    private var controlButtons: some View {
        VStack(spacing: 12) {
            HStack(spacing: 20) {
                Button {
                    Task {
                        if !transcriptionController.isListening {
                            responseSpeaker.stop()
                        }
                        await transcriptionController.toggleRecording()
                    }
                } label: {
                    MicControlButtonGlyph(
                        isListening: transcriptionController.isListening,
                        isDisabled: transcriptionController.isRequestingPermissions
                    )
                }
                .buttonStyle(.plain)
                .disabled(transcriptionController.isRequestingPermissions)
                .accessibilityLabel(transcriptionController.isListening ? "Stop recording" : "Start recording")

                Button {
                    transcriptionController.clearTranscript()
                    model.clearAssistantConversation()
                } label: {
                    CircleActionButtonGlyph(systemName: "xmark", label: "Clear")
                }
                .buttonStyle(.plain)
                .disabled(transcriptionController.transcript.isEmpty)

                Button {
                    Task {
                        await model.queryAssistant(query: transcriptionController.transcript)
                    }
                } label: {
                    CircleActionButtonGlyph(
                        systemName: model.isLoading(.queryAssistant) ? "hourglass" : "paperplane.fill",
                        label: model.isLoading(.queryAssistant) ? "Sending" : "Ask Alfred"
                    )
                }
                .buttonStyle(.plain)
                .disabled(
                    transcriptionController.transcript.isEmpty ||
                        transcriptionController.isListening ||
                        model.isLoading(.queryAssistant)
                )
            }
            .frame(maxWidth: .infinity, alignment: .center)

            Text(transcriptionController.isListening ? "Listening…" : "Tap mic to start")
                .font(.subheadline.weight(.semibold))
                .foregroundStyle(AppTheme.Colors.textSecondary)
                .frame(maxWidth: .infinity, alignment: .center)
        }
        .padding(.top, 90)
    }

    private var assistantResponseView: some View {
        AssistantConversationView(
            messages: model.assistantConversation,
            isLoading: model.isLoading(.queryAssistant)
        )
    }
}

#Preview {
    let clerk = Clerk.preview()
    HomeVoiceTranscriptionSection(model: AppModel(clerk: clerk))
        .padding()
        .appScreenBackground()
}
