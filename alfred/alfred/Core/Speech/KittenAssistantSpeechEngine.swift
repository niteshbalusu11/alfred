import Foundation

@MainActor
final class KittenAssistantSpeechEngine: AssistantSpeechEngine {
    private let synthesizer: AssistantWaveformSynthesizing
    private let audioPlayer: AssistantWaveformPlaying

    init(
        synthesizer: AssistantWaveformSynthesizing = KittenOnnxSynthesizer(),
        audioPlayer: AssistantWaveformPlaying? = nil
    ) {
        self.synthesizer = synthesizer
        if let audioPlayer {
            self.audioPlayer = audioPlayer
        } else {
            self.audioPlayer = KittenWaveformAudioPlayer()
        }

        if let kittenSynthesizer = synthesizer as? KittenOnnxSynthesizer {
            Task(priority: .utility) {
                await kittenSynthesizer.preloadResources()
            }
        }
    }

    var isSpeaking: Bool {
        audioPlayer.isPlaying
    }

    func speak(_ text: String) async throws {
        try Task.checkCancellation()
        let normalizedText = KittenSpeechTextNormalizer.normalize(text)
        guard !normalizedText.isEmpty else { return }

        let samples = try await synthesizer.synthesize(text: normalizedText)
        try Task.checkCancellation()
        try audioPlayer.play(samples: samples)
    }

    func stop() {
        audioPlayer.stop()
    }
}
