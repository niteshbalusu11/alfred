import Foundation

@MainActor
protocol AssistantSpeechEngine: AnyObject {
    var isSpeaking: Bool { get }
    func speak(_ text: String) async throws
    func stop()
}

nonisolated protocol AssistantWaveformSynthesizing: AnyObject {
    func synthesize(text: String) async throws -> [Float]
}

@MainActor
protocol AssistantWaveformPlaying: AnyObject {
    var isPlaying: Bool { get }
    func play(samples: [Float]) throws
    func stop()
}
