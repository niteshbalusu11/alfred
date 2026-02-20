import XCTest
@testable import alfred

@MainActor
final class AssistantResponseSpeakerTests: XCTestCase {
    func testSpeakIgnoresEmptyText() async {
        let speechEngine = MockAssistantSpeechEngine()
        let audioSession = MockAssistantSpeechAudioSessionController()
        let speaker = AssistantResponseSpeaker(
            speechEngine: speechEngine,
            audioSessionController: audioSession
        )

        speaker.speak("   ")
        await Task.yield()

        XCTAssertTrue(speechEngine.spokenTexts.isEmpty)
        XCTAssertEqual(audioSession.prepareCallCount, 0)
    }

    func testSpeakStopsInFlightSpeechBeforeNewUtterance() async {
        let speechEngine = MockAssistantSpeechEngine()
        speechEngine.isSpeakingValue = true
        let audioSession = MockAssistantSpeechAudioSessionController()
        let speaker = AssistantResponseSpeaker(
            speechEngine: speechEngine,
            audioSessionController: audioSession
        )

        speaker.speak("Hello from Alfred")
        await Task.yield()

        XCTAssertEqual(speechEngine.stopCallCount, 1)
        XCTAssertEqual(speechEngine.spokenTexts, ["Hello from Alfred"])
        XCTAssertEqual(audioSession.prepareCallCount, 1)
    }

    func testStopEndsSpeechAndDeactivatesAudioSession() {
        let speechEngine = MockAssistantSpeechEngine()
        speechEngine.isSpeakingValue = true
        let audioSession = MockAssistantSpeechAudioSessionController()
        let speaker = AssistantResponseSpeaker(
            speechEngine: speechEngine,
            audioSessionController: audioSession
        )

        speaker.stop()

        XCTAssertEqual(speechEngine.stopCallCount, 1)
        XCTAssertEqual(audioSession.deactivateCallCount, 1)
    }
}

@MainActor
private final class MockAssistantSpeechEngine: AssistantSpeechEngine {
    var isSpeakingValue = false
    var spokenTexts: [String] = []
    var stopCallCount = 0

    var isSpeaking: Bool {
        isSpeakingValue
    }

    func speak(_ text: String) async throws {
        spokenTexts.append(text)
        isSpeakingValue = true
    }

    func stop() {
        stopCallCount += 1
        isSpeakingValue = false
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
