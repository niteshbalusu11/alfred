import AVFoundation
import XCTest
@testable import alfred

@MainActor
final class AssistantResponseSpeakerTests: XCTestCase {
    func testSpeakIgnoresEmptyText() {
        let synthesizer = MockAssistantSpeechSynthesizer()
        let audioSession = MockAssistantSpeechAudioSessionController()
        let speaker = AssistantResponseSpeaker(
            synthesizer: synthesizer,
            audioSessionController: audioSession
        )

        speaker.speak("   ")

        XCTAssertTrue(synthesizer.spokenUtterances.isEmpty)
        XCTAssertEqual(audioSession.prepareCallCount, 0)
    }

    func testSpeakStopsInFlightSpeechBeforeNewUtterance() {
        let synthesizer = MockAssistantSpeechSynthesizer()
        synthesizer.isSpeakingValue = true
        let audioSession = MockAssistantSpeechAudioSessionController()
        let speaker = AssistantResponseSpeaker(
            synthesizer: synthesizer,
            audioSessionController: audioSession
        )

        speaker.speak("Hello from Alfred")

        XCTAssertEqual(synthesizer.stopCallCount, 1)
        XCTAssertEqual(synthesizer.spokenUtterances.count, 1)
        XCTAssertEqual(synthesizer.spokenUtterances.first?.speechString, "Hello from Alfred")
        XCTAssertEqual(audioSession.prepareCallCount, 1)
    }

    func testStopEndsSpeechAndDeactivatesAudioSession() {
        let synthesizer = MockAssistantSpeechSynthesizer()
        synthesizer.isSpeakingValue = true
        let audioSession = MockAssistantSpeechAudioSessionController()
        let speaker = AssistantResponseSpeaker(
            synthesizer: synthesizer,
            audioSessionController: audioSession
        )

        speaker.stop()

        XCTAssertEqual(synthesizer.stopCallCount, 1)
        XCTAssertEqual(audioSession.deactivateCallCount, 1)
    }
}

private final class MockAssistantSpeechSynthesizer: AssistantSpeechSynthesizing {
    var isSpeakingValue = false
    var spokenUtterances: [AVSpeechUtterance] = []
    var stopCallCount = 0

    var isSpeaking: Bool {
        isSpeakingValue
    }

    func speak(_ utterance: AVSpeechUtterance) {
        spokenUtterances.append(utterance)
        isSpeakingValue = true
    }

    @discardableResult
    func stopSpeaking(at boundary: AVSpeechBoundary) -> Bool {
        stopCallCount += 1
        isSpeakingValue = false
        return true
    }
}

private final class MockAssistantSpeechAudioSessionController: AssistantSpeechAudioSessionControlling {
    private(set) var prepareCallCount = 0
    private(set) var deactivateCallCount = 0

    func prepareForPlayback() throws {
        prepareCallCount += 1
    }

    func deactivate() throws {
        deactivateCallCount += 1
    }
}
