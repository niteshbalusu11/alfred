import Foundation

enum KittenSpeechModelStoreError: Error {
    case invalidModelResponse
}

struct KittenSpeechModelStore {
    nonisolated private static let modelFileName = "kitten_tts_micro_v0_8.onnx"
    nonisolated private static let remoteModelURL = URL(
        string: "https://huggingface.co/KittenML/kitten-tts-micro-0.8/resolve/main/kitten_tts_micro_v0_8.onnx"
    )!

    nonisolated init() {}

    nonisolated func resolveModelPath() async throws -> String {
        let fileManager = FileManager.default

        if let bundledModelPath = bundledModelPath() {
            return bundledModelPath
        }

        let localURL = try localModelURL(fileManager: fileManager)
        if fileManager.fileExists(atPath: localURL.path) {
            return localURL.path
        }

        let (temporaryURL, response) = try await URLSession.shared.download(from: Self.remoteModelURL)
        guard let httpResponse = response as? HTTPURLResponse,
              (200 ..< 300).contains(httpResponse.statusCode) else {
            throw KittenSpeechModelStoreError.invalidModelResponse
        }

        if fileManager.fileExists(atPath: localURL.path) {
            try fileManager.removeItem(at: localURL)
        }

        try fileManager.moveItem(at: temporaryURL, to: localURL)
        // File protection is best-effort; unsupported environments (e.g. simulator)
        // should still be able to use the downloaded model.
        try? fileManager.setAttributes(
            [.protectionKey: FileProtectionType.completeUntilFirstUserAuthentication],
            ofItemAtPath: localURL.path
        )

        return localURL.path
    }

    nonisolated private func localModelURL(fileManager: FileManager) throws -> URL {
        let appSupport = try fileManager.url(
            for: .applicationSupportDirectory,
            in: .userDomainMask,
            appropriateFor: nil,
            create: true
        )
        let directory = appSupport.appendingPathComponent("KittenTTS", isDirectory: true)
        if !fileManager.fileExists(atPath: directory.path) {
            try fileManager.createDirectory(at: directory, withIntermediateDirectories: true)
        }

        return directory.appendingPathComponent(Self.modelFileName)
    }

    nonisolated private func bundledModelPath() -> String? {
        if let directPath = Bundle.main.path(forResource: "kitten_tts_micro_v0_8", ofType: "onnx") {
            return directPath
        }

        return Bundle.main.path(
            forResource: "kitten_tts_micro_v0_8",
            ofType: "onnx",
            inDirectory: "KittenTTS"
        )
    }

}
