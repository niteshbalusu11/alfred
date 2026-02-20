import Foundation
import OnnxRuntimeBindings

enum KittenOnnxSynthesizerError: Error {
    case missingWaveformOutput
    case unsupportedWaveformBuffer
}

actor KittenOnnxSynthesizer: AssistantWaveformSynthesizing {
    private let modelStore: KittenSpeechModelStore
    private let styleStore: KittenVoiceStyleStore
    private let phonemizer: KittenEnglishPhonemizer
    private let voiceID: KittenVoiceID
    private let baseSpeed: Float

    private var env: ORTEnv?
    private var session: ORTSession?
    private var loadedModelPath: String?
    private var styleMatrices: [KittenVoiceID: KittenVoiceStyleMatrix] = [:]

    init(
        modelStore: KittenSpeechModelStore = KittenSpeechModelStore(),
        styleStore: KittenVoiceStyleStore = KittenVoiceStyleStore(),
        phonemizer: KittenEnglishPhonemizer = KittenEnglishPhonemizer(),
        voiceID: KittenVoiceID = KittenVoiceStyleStore.defaultVoiceID,
        baseSpeed: Float = 1.28
    ) {
        self.modelStore = modelStore
        self.styleStore = styleStore
        self.phonemizer = phonemizer
        self.voiceID = voiceID
        self.baseSpeed = baseSpeed
    }

    func synthesize(text: String) async throws -> [Float] {
        try Task.checkCancellation()
        let chunks = Self.chunked(text: text)
        guard !chunks.isEmpty else { return [] }

        let activeSession = try await activeSession()
        let activeStyleMatrix = try loadStyleMatrix(for: voiceID)
        let speed = Self.effectiveSpeed(baseSpeed: baseSpeed)

        var allSamples: [Float] = []
        allSamples.reserveCapacity(chunks.count * 24_000)

        for (chunkIndex, chunk) in chunks.enumerated() {
            try Task.checkCancellation()
            let phonemeText = (try? await phonemizer.phonemize(chunk)) ?? chunk
            let inputTokens = Self.tokenize(phonemeText)
            guard !inputTokens.isEmpty else { continue }

            let styleVector = activeStyleMatrix.styleVector(forTextLength: inputTokens.count)
            let chunkSpeed = Self.expressiveSpeed(baseSpeed: speed, for: chunk)
            let chunkSamples = try run(
                session: activeSession,
                tokens: inputTokens,
                styleVector: styleVector,
                speed: chunkSpeed
            )

            guard !chunkSamples.isEmpty else { continue }

            Self.appendChunk(chunkSamples, to: &allSamples)

            if chunkIndex < chunks.count - 1 {
                let pause = Self.pauseSampleCount(after: chunk)
                if pause > 0 {
                    allSamples.append(contentsOf: repeatElement(0, count: pause))
                }
            }
        }

        try Task.checkCancellation()
        return allSamples
    }

    func preloadResources() async {
        do {
            _ = try await activeSession()
            _ = try loadStyleMatrix(for: voiceID)
            _ = try await phonemizer.phonemize("Hello")
        } catch {
            // Warm-up is best-effort; synthesis path handles hard failures.
        }
    }

    private func activeSession() async throws -> ORTSession {
        let modelPath = try await modelStore.resolveModelPath()

        if let session,
           let loadedModelPath,
           loadedModelPath == modelPath {
            return session
        }

        let env = try ORTEnv(loggingLevel: .warning)
        let options = try ORTSessionOptions()
        try options.setIntraOpNumThreads(2)

        let session = try ORTSession(env: env, modelPath: modelPath, sessionOptions: options)

        self.env = env
        self.session = session
        self.loadedModelPath = modelPath

        return session
    }

    private func loadStyleMatrix(for voiceID: KittenVoiceID) throws -> KittenVoiceStyleMatrix {
        if let styleMatrix = styleMatrices[voiceID] {
            return styleMatrix
        }

        let matrix = try styleStore.loadVoiceMatrix(for: voiceID)
        styleMatrices[voiceID] = matrix
        return matrix
    }

    private func run(
        session: ORTSession,
        tokens: [Int64],
        styleVector: [Float],
        speed: Float
    ) throws -> [Float] {
        var inputTokenBuffer = tokens
        let inputTokenData = NSMutableData(
            bytes: &inputTokenBuffer,
            length: inputTokenBuffer.count * MemoryLayout<Int64>.size
        )

        var inputStyleBuffer = styleVector
        let inputStyleData = NSMutableData(
            bytes: &inputStyleBuffer,
            length: inputStyleBuffer.count * MemoryLayout<Float>.size
        )

        var inputSpeed = speed
        let inputSpeedData = NSMutableData(bytes: &inputSpeed, length: MemoryLayout<Float>.size)

        let inputIDs = try ORTValue(
            tensorData: inputTokenData,
            elementType: .int64,
            shape: [1, NSNumber(value: tokens.count)]
        )
        let style = try ORTValue(
            tensorData: inputStyleData,
            elementType: .float,
            shape: [1, NSNumber(value: styleVector.count)]
        )
        let speedValue = try ORTValue(
            tensorData: inputSpeedData,
            elementType: .float,
            shape: [1]
        )

        let outputs = try session.run(
            withInputs: [
                "input_ids": inputIDs,
                "style": style,
                "speed": speedValue,
            ],
            outputNames: Set(["waveform"]),
            runOptions: nil
        )

        guard let waveformOutput = outputs["waveform"] else {
            throw KittenOnnxSynthesizerError.missingWaveformOutput
        }

        let waveformData = try waveformOutput.tensorData()
        guard waveformData.length % MemoryLayout<Float>.size == 0 else {
            throw KittenOnnxSynthesizerError.unsupportedWaveformBuffer
        }

        let sampleCount = waveformData.length / MemoryLayout<Float>.size
        var samples = [Float](repeating: 0, count: sampleCount)
        samples.withUnsafeMutableBytes { sampleBuffer in
            guard let baseAddress = sampleBuffer.baseAddress else { return }
            waveformData.getBytes(baseAddress, length: waveformData.length)
        }

        return samples
    }

    nonisolated private static func effectiveSpeed(baseSpeed: Float) -> Float {
        if baseSpeed <= 0 {
            return 1.0
        }
        return baseSpeed
    }

    nonisolated private static func expressiveSpeed(baseSpeed: Float, for chunk: String) -> Float {
        var adjusted = baseSpeed

        if chunk.contains("?") {
            adjusted *= 1.08
        }
        if chunk.contains("!") {
            adjusted *= 1.06
        }
        if chunk.contains(",") || chunk.contains(";") || chunk.contains(":") {
            adjusted *= 0.97
        }
        if chunk.count < 32 {
            adjusted *= 1.05
        }

        return min(max(adjusted, 0.92), 1.7)
    }

    nonisolated private static func pauseSampleCount(after chunk: String) -> Int {
        let sampleRate = 24_000
        guard let last = chunk.trimmingCharacters(in: .whitespacesAndNewlines).last else {
            return 0
        }

        switch last {
        case ",":
            return Int(Double(sampleRate) * 0.06)
        case ";", ":":
            return Int(Double(sampleRate) * 0.08)
        case "?", "!":
            return Int(Double(sampleRate) * 0.09)
        case ".":
            return Int(Double(sampleRate) * 0.11)
        default:
            return 0
        }
    }

    nonisolated private static func appendChunk(_ chunk: [Float], to allSamples: inout [Float]) {
        guard !chunk.isEmpty else { return }
        guard !allSamples.isEmpty else {
            allSamples.append(contentsOf: chunk)
            return
        }

        let crossfadeSampleCount = min(384, allSamples.count, chunk.count)
        if crossfadeSampleCount == 0 {
            allSamples.append(contentsOf: chunk)
            return
        }

        let fadeStartIndex = allSamples.count - crossfadeSampleCount
        for index in 0 ..< crossfadeSampleCount {
            let blend = Float(index) / Float(crossfadeSampleCount)
            let existing = allSamples[fadeStartIndex + index]
            let incoming = chunk[index]
            allSamples[fadeStartIndex + index] = existing * (1 - blend) + incoming * blend
        }

        if crossfadeSampleCount < chunk.count {
            allSamples.append(contentsOf: chunk[crossfadeSampleCount...])
        }
    }

    nonisolated private static func chunked(text: String, maxLength: Int = 400) -> [String] {
        let normalized = text
            .trimmingCharacters(in: .whitespacesAndNewlines)
            .replacingOccurrences(of: #"\s+"#, with: " ", options: .regularExpression)

        guard !normalized.isEmpty else { return [] }

        let sentences = normalized
            .components(separatedBy: CharacterSet(charactersIn: ".!?"))
            .map { $0.trimmingCharacters(in: .whitespacesAndNewlines) }
            .filter { !$0.isEmpty }

        guard !sentences.isEmpty else {
            return [ensurePunctuation(normalized)]
        }

        var chunks: [String] = []
        for sentence in sentences {
            if sentence.count <= maxLength {
                chunks.append(ensurePunctuation(sentence))
                continue
            }

            var currentChunk = ""
            for token in sentence.split(separator: " ") {
                if currentChunk.isEmpty {
                    currentChunk = String(token)
                    continue
                }

                let candidate = currentChunk + " " + token
                if candidate.count <= maxLength {
                    currentChunk = candidate
                } else {
                    chunks.append(ensurePunctuation(currentChunk))
                    currentChunk = String(token)
                }
            }

            if !currentChunk.isEmpty {
                chunks.append(ensurePunctuation(currentChunk))
            }
        }

        return chunks
    }

    nonisolated private static func ensurePunctuation(_ text: String) -> String {
        guard let last = text.last else { return text }
        if ".!?,;:".contains(last) {
            return text
        }
        return text + "."
    }

    nonisolated private static func tokenize(_ text: String) -> [Int64] {
        var tokens = [Int64]()
        tokens.reserveCapacity(text.count + 2)
        tokens.append(0)

        for character in text {
            if let index = textSymbolIndex[character] {
                tokens.append(Int64(index))
            }
        }

        tokens.append(0)
        return tokens
    }

    nonisolated private static let textSymbolIndex: [Character: Int] = {
        let punctuation = Array(";:,.!?¡¿—…\"«»\"\" ")
        let letters = Array("ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz")
        let lettersIPA = Array("ɑɐɒæɓʙβɔɕçɗɖðʤəɘɚɛɜɝɞɟʄɡɠɢʛɦɧħɥʜɨɪʝɭɬɫɮʟɱɯɰŋɳɲɴøɵɸθœɶʘɹɺɾɻʀʁɽʂʃʈʧʉʊʋⱱʌɣɤʍχʎʏʑʐʒʔʡʕʢǀǁǂǃˈˌːˑʼʴʰʱʲʷˠˤ˞↓↑→↗↘'̩'ᵻ")

        var symbols = [Character]()
        symbols.reserveCapacity(1 + punctuation.count + letters.count + lettersIPA.count)
        symbols.append("$")
        symbols.append(contentsOf: punctuation)
        symbols.append(contentsOf: letters)
        symbols.append(contentsOf: lettersIPA)

        var index: [Character: Int] = [:]
        for (offset, symbol) in symbols.enumerated() {
            index[symbol] = offset
        }
        return index
    }()

}
