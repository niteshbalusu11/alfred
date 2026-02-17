import AVFoundation
import Foundation
import Speech

enum VoiceRecognitionAuthorization {
    case authorized
    case denied
    case restricted
    case unavailable
}

@MainActor
protocol VoiceLiveSpeechRecognizing: AnyObject {
    func requestAuthorization() async -> VoiceRecognitionAuthorization
    func startLiveTranscription(
        onUpdate: @escaping @MainActor @Sendable (String) -> Void,
        onFailure: @escaping @MainActor @Sendable (String) -> Void
    ) throws
    func stop()
}

enum VoiceLiveSpeechRecognizerError: Error {
    case unavailable
    case startFailed
}

@MainActor
final class AppleVoiceLiveSpeechRecognizer: VoiceLiveSpeechRecognizing {
    private let speechRecognizer: SFSpeechRecognizer?
    private let audioEngine: AVAudioEngine
    private var recognitionRequest: SFSpeechAudioBufferRecognitionRequest?
    private var recognitionTask: SFSpeechRecognitionTask?

    init(locale: Locale = .current, audioEngine: AVAudioEngine = AVAudioEngine()) {
        self.speechRecognizer = SFSpeechRecognizer(locale: locale)
        self.audioEngine = audioEngine
    }

    func requestAuthorization() async -> VoiceRecognitionAuthorization {
        guard speechRecognizer != nil else {
            return .unavailable
        }

        let speechStatus = await requestSpeechRecognitionAuthorization()
        switch speechStatus {
        case .authorized:
            break
        case .restricted:
            return .restricted
        case .denied, .notDetermined:
            return .denied
        @unknown default:
            return .unavailable
        }

        let microphoneAuthorized = await requestMicrophoneAuthorization()
        return microphoneAuthorized ? .authorized : .denied
    }

    func startLiveTranscription(
        onUpdate: @escaping @MainActor @Sendable (String) -> Void,
        onFailure: @escaping @MainActor @Sendable (String) -> Void
    ) throws {
        guard let speechRecognizer,
              speechRecognizer.isAvailable,
              speechRecognizer.supportsOnDeviceRecognition
        else {
            throw VoiceLiveSpeechRecognizerError.unavailable
        }

        stop()

        let request = SFSpeechAudioBufferRecognitionRequest()
        request.shouldReportPartialResults = true
        request.requiresOnDeviceRecognition = true
        recognitionRequest = request

        let audioSession = AVAudioSession.sharedInstance()

        do {
            try audioSession.setCategory(.record, mode: .measurement, options: [.duckOthers])
            try audioSession.setActive(true, options: .notifyOthersOnDeactivation)
        } catch {
            throw VoiceLiveSpeechRecognizerError.startFailed
        }

        let inputNode = audioEngine.inputNode
        let format = inputNode.outputFormat(forBus: 0)
        inputNode.removeTap(onBus: 0)
        inputNode.installTap(onBus: 0, bufferSize: 1024, format: format) { [weak self] buffer, _ in
            self?.recognitionRequest?.append(buffer)
        }

        audioEngine.prepare()

        do {
            try audioEngine.start()
        } catch {
            stop()
            throw VoiceLiveSpeechRecognizerError.startFailed
        }

        recognitionTask = speechRecognizer.recognitionTask(with: request) { [weak self] result, error in
            guard let self else { return }

            if let result {
                Task { @MainActor in
                    onUpdate(result.bestTranscription.formattedString)
                }

                if result.isFinal {
                    self.stop()
                }
            }

            if let error {
                Task { @MainActor in
                    onFailure(Self.humanReadableError(from: error))
                }
                self.stop()
            }
        }
    }

    func stop() {
        audioEngine.stop()
        audioEngine.inputNode.removeTap(onBus: 0)
        recognitionRequest?.endAudio()
        recognitionTask?.cancel()
        recognitionRequest = nil
        recognitionTask = nil
        try? AVAudioSession.sharedInstance().setActive(false, options: .notifyOthersOnDeactivation)
    }

    private func requestSpeechRecognitionAuthorization() async -> SFSpeechRecognizerAuthorizationStatus {
        await withCheckedContinuation { continuation in
            SFSpeechRecognizer.requestAuthorization { status in
                continuation.resume(returning: status)
            }
        }
    }

    private func requestMicrophoneAuthorization() async -> Bool {
        await withCheckedContinuation { continuation in
            AVAudioApplication.requestRecordPermission { granted in
                continuation.resume(returning: granted)
            }
        }
    }

    private static func humanReadableError(from error: any Error) -> String {
        let nsError = error as NSError
        if let localizedFailureReason = nsError.localizedFailureReason,
           !localizedFailureReason.isEmpty
        {
            return localizedFailureReason
        }
        if !nsError.localizedDescription.isEmpty {
            return nsError.localizedDescription
        }
        return "Speech recognition failed."
    }
}
