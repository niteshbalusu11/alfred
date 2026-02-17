import AVFoundation
import Foundation

nonisolated protocol AssistantSpeechSynthesizing: AnyObject {
    var isSpeaking: Bool { get }
    func speak(_ utterance: AVSpeechUtterance)
    @discardableResult
    func stopSpeaking(at boundary: AVSpeechBoundary) -> Bool
}

extension AVSpeechSynthesizer: AssistantSpeechSynthesizing {}

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
        try session.setCategory(.playback, mode: .spokenAudio, options: [.duckOthers])
        try session.setActive(true, options: .notifyOthersOnDeactivation)
    }

    func deactivate() throws {
        try session.setActive(false, options: .notifyOthersOnDeactivation)
    }
}

nonisolated final class AssistantResponseSpeaker {
    private let synthesizer: AssistantSpeechSynthesizing
    private let audioSessionController: AssistantSpeechAudioSessionControlling

    @MainActor
    convenience init() {
        self.init(
            synthesizer: AVSpeechSynthesizer(),
            audioSessionController: AssistantSpeechAudioSessionController()
        )
    }

    init(
        synthesizer: AssistantSpeechSynthesizing,
        audioSessionController: AssistantSpeechAudioSessionControlling
    ) {
        self.synthesizer = synthesizer
        self.audioSessionController = audioSessionController
    }

    @MainActor
    func speak(_ text: String) {
        let trimmed = text.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return }

        if synthesizer.isSpeaking {
            _ = synthesizer.stopSpeaking(at: .immediate)
        }

        try? audioSessionController.prepareForPlayback()

        let utterance = AVSpeechUtterance(string: trimmed)
        utterance.voice = AVSpeechSynthesisVoice(language: Locale.current.identifier)
            ?? AVSpeechSynthesisVoice(language: "en-US")
        synthesizer.speak(utterance)
    }

    @MainActor
    func stop() {
        if synthesizer.isSpeaking {
            _ = synthesizer.stopSpeaking(at: .immediate)
        }
        try? audioSessionController.deactivate()
    }
}
