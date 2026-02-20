import Foundation

enum KittenVoiceStyleStoreError: Error {
    case missingStyleData
    case invalidStyleDataSize
}

enum KittenVoiceID: String, CaseIterable {
    case bella = "expr-voice-2-f"
    case jasper = "expr-voice-2-m"
    case luna = "expr-voice-3-f"
    case bruno = "expr-voice-3-m"
    case rosie = "expr-voice-4-f"
    case hugo = "expr-voice-4-m"
    case kiki = "expr-voice-5-f"
    case leo = "expr-voice-5-m"
}

struct KittenVoiceStyleMatrix {
    nonisolated static let rowCount = 400
    nonisolated static let columnCount = 256

    private let values: [Float]

    nonisolated init(values: [Float]) {
        self.values = values
    }

    nonisolated func styleVector(forTextLength textLength: Int) -> [Float] {
        let clampedIndex = max(0, min(textLength, Self.rowCount - 1))
        let start = clampedIndex * Self.columnCount
        let end = start + Self.columnCount
        return Array(values[start ..< end])
    }
}

struct KittenVoiceStyleStore {
    nonisolated static let defaultVoiceID: KittenVoiceID = .hugo

    nonisolated init() {}

    nonisolated func loadVoiceMatrix(for voiceID: KittenVoiceID) throws -> KittenVoiceStyleMatrix {
        guard let url = bundledStyleURL(for: voiceID) else {
            throw KittenVoiceStyleStoreError.missingStyleData
        }

        let data = try Data(contentsOf: url)
        let expectedBytes = KittenVoiceStyleMatrix.rowCount
            * KittenVoiceStyleMatrix.columnCount
            * MemoryLayout<Float>.size

        guard data.count == expectedBytes else {
            throw KittenVoiceStyleStoreError.invalidStyleDataSize
        }

        let floatCount = data.count / MemoryLayout<Float>.size
        var values = [Float](repeating: 0, count: floatCount)
        _ = values.withUnsafeMutableBytes { rawBuffer in
            data.copyBytes(to: rawBuffer)
        }

        return KittenVoiceStyleMatrix(values: values)
    }

    nonisolated private func bundledStyleURL(for voiceID: KittenVoiceID) -> URL? {
        let resourceName = styleResourceName(for: voiceID)
        if let directURL = Bundle.main.url(
            forResource: resourceName,
            withExtension: "bin"
        ) {
            return directURL
        }

        return Bundle.main.url(
            forResource: resourceName,
            withExtension: "bin",
            subdirectory: "KittenTTS"
        )
    }

    nonisolated private func styleResourceName(for voiceID: KittenVoiceID) -> String {
        "\(voiceID.rawValue.replacingOccurrences(of: "-", with: "_"))_style_400x256_f32"
    }
}
