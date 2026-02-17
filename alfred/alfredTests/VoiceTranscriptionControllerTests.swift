import XCTest
@testable import alfred

@MainActor
final class VoiceTranscriptionControllerTests: XCTestCase {
    func testStartRecordingDeniedPermissionSetsPermissionDeniedState() async {
        let recognizer = MockVoiceLiveSpeechRecognizer()
        recognizer.authorization = .denied
        let controller = VoiceTranscriptionController(recognizer: recognizer)

        await controller.startRecording()

        XCTAssertEqual(controller.status, .permissionDenied)
        XCTAssertFalse(recognizer.didStart)
    }

    func testStartRecordingAuthorizedBeginsListeningAndUpdatesTranscript() async {
        let recognizer = MockVoiceLiveSpeechRecognizer()
        recognizer.authorization = .authorized
        let controller = VoiceTranscriptionController(recognizer: recognizer)

        await controller.startRecording()
        recognizer.emitTranscript("hello world")

        XCTAssertEqual(controller.status, .listening)
        XCTAssertEqual(controller.transcript, "hello world")
    }

    func testStopRecordingResetsToIdle() async {
        let recognizer = MockVoiceLiveSpeechRecognizer()
        recognizer.authorization = .authorized
        let controller = VoiceTranscriptionController(recognizer: recognizer)

        await controller.startRecording()
        controller.stopRecording()

        XCTAssertTrue(recognizer.didStop)
        XCTAssertEqual(controller.status, .idle)
    }

    func testStartRecordingHandlesStartFailure() async {
        let recognizer = MockVoiceLiveSpeechRecognizer()
        recognizer.authorization = .authorized
        recognizer.startError = VoiceLiveSpeechRecognizerError.startFailed
        let controller = VoiceTranscriptionController(recognizer: recognizer)

        await controller.startRecording()

        XCTAssertEqual(controller.status, .failed(message: "Could not start transcription. Try again."))
    }

    func testRecognizerFailureTransitionsToFailedState() async {
        let recognizer = MockVoiceLiveSpeechRecognizer()
        recognizer.authorization = .authorized
        let controller = VoiceTranscriptionController(recognizer: recognizer)

        await controller.startRecording()
        recognizer.emitFailure("Speech recognition failed.")

        XCTAssertEqual(controller.status, .failed(message: "Speech recognition failed."))
    }

    func testRecognizerFailureAfterStopIsIgnored() async {
        let recognizer = MockVoiceLiveSpeechRecognizer()
        recognizer.authorization = .authorized
        let controller = VoiceTranscriptionController(recognizer: recognizer)

        await controller.startRecording()
        controller.stopRecording()
        recognizer.emitFailure("Speech recognition failed.")

        XCTAssertEqual(controller.status, .idle)
    }
}

@MainActor
private final class MockVoiceLiveSpeechRecognizer: VoiceLiveSpeechRecognizing {
    var authorization: VoiceRecognitionAuthorization = .authorized
    var startError: Error?
    var didStart = false
    var didStop = false

    private var onUpdate: (@MainActor @Sendable (String) -> Void)?
    private var onFailure: (@MainActor @Sendable (String) -> Void)?

    func requestAuthorization() async -> VoiceRecognitionAuthorization {
        authorization
    }

    func startLiveTranscription(
        onUpdate: @escaping @MainActor @Sendable (String) -> Void,
        onFailure: @escaping @MainActor @Sendable (String) -> Void
    ) throws {
        didStart = true
        self.onUpdate = onUpdate
        self.onFailure = onFailure
        if let startError {
            throw startError
        }
    }

    func stop() {
        didStop = true
    }

    func emitTranscript(_ text: String) {
        onUpdate?(text)
    }

    func emitFailure(_ message: String) {
        onFailure?(message)
    }
}
