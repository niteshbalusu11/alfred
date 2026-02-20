import AVFoundation
import Foundation

nonisolated protocol AssistantSpeechAudioSessionControlling: AnyObject {
    func prepareForPlayback() throws
    func deactivate() throws
}

nonisolated final class AssistantSpeechAudioSessionController: AssistantSpeechAudioSessionControlling {
    private let session: AVAudioSession

    init(session: AVAudioSession = .sharedInstance()) {
        self.session = session
    }

    func prepareForPlayback() throws {
        try session.setCategory(
            .playAndRecord,
            mode: .spokenAudio,
            options: [.duckOthers, .defaultToSpeaker, .allowBluetoothHFP]
        )
        try session.setActive(true, options: .notifyOthersOnDeactivation)
    }

    func deactivate() throws {
        try session.setActive(false, options: .notifyOthersOnDeactivation)
    }
}

nonisolated final class AssistantResponseSpeaker {
    private let speechEngine: AssistantSpeechEngine
    private let audioSessionController: AssistantSpeechAudioSessionControlling
    private var speechTask: Task<Void, Never>?

    @MainActor
    convenience init() {
        self.init(
            speechEngine: KittenAssistantSpeechEngine(),
            audioSessionController: AssistantSpeechAudioSessionController()
        )
    }

    @MainActor
    init(
        speechEngine: AssistantSpeechEngine,
        audioSessionController: AssistantSpeechAudioSessionControlling
    ) {
        self.speechEngine = speechEngine
        self.audioSessionController = audioSessionController
    }

    @MainActor
    func speak(_ text: String) {
        let trimmed = text.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return }

        speechTask?.cancel()

        if speechEngine.isSpeaking {
            speechEngine.stop()
        }

        do {
            try audioSessionController.prepareForPlayback()
        } catch {
            AppLogger.warning("Assistant speech audio session setup failed: \(error.localizedDescription)")
        }

        speechTask = Task { [speechEngine] in
            do {
                try await speechEngine.speak(trimmed)
            } catch is CancellationError {
                // Canceling in-flight speech is expected when new responses arrive.
            } catch {
                AppLogger.error("Assistant speech synthesis failed: \(error.localizedDescription)")
            }
        }
    }

    @MainActor
    func stop() {
        speechTask?.cancel()
        speechTask = nil

        if speechEngine.isSpeaking {
            speechEngine.stop()
        }

        do {
            try audioSessionController.deactivate()
        } catch {
            AppLogger.warning("Assistant speech audio session deactivate failed: \(error.localizedDescription)")
        }
    }

    deinit {
        speechTask?.cancel()
    }
}
