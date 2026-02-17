import Combine
import Foundation

@MainActor
final class VoiceTranscriptionController: ObservableObject {
    enum Status: Equatable {
        case idle
        case requestingPermissions
        case listening
        case permissionDenied
        case restricted
        case unavailable
        case failed(message: String)
    }

    @Published private(set) var transcript = ""
    @Published private(set) var status: Status = .idle

    private let recognizer: VoiceLiveSpeechRecognizing

    convenience init() {
        self.init(recognizer: AppleVoiceLiveSpeechRecognizer())
    }

    init(recognizer: VoiceLiveSpeechRecognizing) {
        self.recognizer = recognizer
    }

    var isListening: Bool {
        if case .listening = status {
            return true
        }
        return false
    }

    var isRequestingPermissions: Bool {
        if case .requestingPermissions = status {
            return true
        }
        return false
    }

    func toggleRecording() async {
        if isListening {
            stopRecording()
            return
        }
        await startRecording()
    }

    func startRecording() async {
        guard !isListening, !isRequestingPermissions else { return }

        status = .requestingPermissions
        let authorization = await recognizer.requestAuthorization()

        switch authorization {
        case .authorized:
            break
        case .denied:
            status = .permissionDenied
            return
        case .restricted:
            status = .restricted
            return
        case .unavailable:
            status = .unavailable
            return
        }

        transcript = ""

        do {
            try recognizer.startLiveTranscription(
                onUpdate: { [weak self] updatedText in
                    self?.transcript = updatedText
                },
                onFailure: { [weak self] errorMessage in
                    guard let self else { return }
                    guard case .listening = self.status else {
                        return
                    }
                    self.status = .failed(message: errorMessage)
                }
            )
            status = .listening
        } catch {
            status = .failed(message: "Could not start transcription. Try again.")
        }
    }

    func stopRecording() {
        recognizer.stop()
        if case .listening = status {
            status = .idle
        }
    }

    func clearTranscript() {
        transcript = ""
    }
}
