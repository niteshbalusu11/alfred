import AVFoundation
import Foundation

enum KittenWaveformAudioPlayerError: Error {
    case failedToCreatePlayer
}

@MainActor
final class KittenWaveformAudioPlayer: NSObject, AssistantWaveformPlaying, AVAudioPlayerDelegate {
    private let sampleRate: Int = 24_000
    private var player: AVAudioPlayer?

    var isPlaying: Bool {
        player?.isPlaying ?? false
    }

    func play(samples: [Float]) throws {
        guard !samples.isEmpty else { return }

        stop()

        let wavData = Self.makeWaveData(samples: samples, sampleRate: sampleRate)
        let player = try AVAudioPlayer(data: wavData)
        player.delegate = self

        guard player.prepareToPlay() && player.play() else {
            throw KittenWaveformAudioPlayerError.failedToCreatePlayer
        }

        self.player = player
    }

    func stop() {
        player?.stop()
        player = nil
    }

    func audioPlayerDidFinishPlaying(_ player: AVAudioPlayer, successfully flag: Bool) {
        self.player = nil
    }

    nonisolated private static func makeWaveData(samples: [Float], sampleRate: Int) -> Data {
        let prepared = normalizeAndApplyEndpointFades(samples, sampleRate: sampleRate)
        let sanitized: [Int16] = prepared.map { sample in
            let clamped = max(-1.0, min(1.0, sample))
            return Int16(clamped * Float(Int16.max))
        }

        let channelCount: UInt16 = 1
        let bitsPerSample: UInt16 = 16
        let bytesPerSample = Int(bitsPerSample / 8)
        let dataSize = sanitized.count * bytesPerSample
        let byteRate = UInt32(sampleRate * Int(channelCount) * bytesPerSample)
        let blockAlign = UInt16(Int(channelCount) * bytesPerSample)

        var data = Data(capacity: 44 + dataSize)
        data.append("RIFF".data(using: .ascii)!)
        appendLittleEndian(UInt32(36 + dataSize), to: &data)
        data.append("WAVE".data(using: .ascii)!)

        data.append("fmt ".data(using: .ascii)!)
        appendLittleEndian(UInt32(16), to: &data)
        appendLittleEndian(UInt16(1), to: &data)
        appendLittleEndian(channelCount, to: &data)
        appendLittleEndian(UInt32(sampleRate), to: &data)
        appendLittleEndian(byteRate, to: &data)
        appendLittleEndian(blockAlign, to: &data)
        appendLittleEndian(bitsPerSample, to: &data)

        data.append("data".data(using: .ascii)!)
        appendLittleEndian(UInt32(dataSize), to: &data)

        for sample in sanitized {
            appendLittleEndian(sample, to: &data)
        }

        return data
    }

    nonisolated private static func normalizeAndApplyEndpointFades(
        _ samples: [Float],
        sampleRate: Int
    ) -> [Float] {
        guard !samples.isEmpty else { return samples }

        var prepared = samples.map { $0.isFinite ? $0 : 0 }
        prepared = trimLowEnergyEdges(prepared, sampleRate: sampleRate)
        if prepared.isEmpty {
            return samples
        }

        removeDCOffset(&prepared)

        let peak = prepared.reduce(Float(0)) { partial, sample in
            max(partial, abs(sample))
        }

        if peak > 0.95 {
            let gain = 0.95 / peak
            for index in prepared.indices {
                prepared[index] *= gain
            }
        }

        let fadeSampleCount = min(480, prepared.count / 2)
        if fadeSampleCount > 0 {
            for index in 0 ..< fadeSampleCount {
                let gain = Float(index) / Float(fadeSampleCount)
                prepared[index] *= gain
                let trailingIndex = prepared.count - 1 - index
                prepared[trailingIndex] *= gain
            }
        }

        return prepared
    }

    nonisolated private static func trimLowEnergyEdges(_ samples: [Float], sampleRate: Int) -> [Float] {
        guard samples.count > 2 else { return samples }

        let peak = samples.reduce(Float(0)) { partial, sample in
            max(partial, abs(sample))
        }
        let energyThreshold = max(0.0025, peak * 0.02)
        let maxTrimSamples = min(sampleRate / 25, samples.count / 8)

        var leadingTrim = 0
        while leadingTrim < maxTrimSamples && abs(samples[leadingTrim]) < energyThreshold {
            leadingTrim += 1
        }

        var trailingTrim = 0
        while trailingTrim < maxTrimSamples
            && abs(samples[samples.count - 1 - trailingTrim]) < energyThreshold {
            trailingTrim += 1
        }

        let startIndex = leadingTrim
        let endExclusive = samples.count - trailingTrim
        guard startIndex < endExclusive else { return samples }

        let trimmed = Array(samples[startIndex ..< endExclusive])
        return trimmed.count >= samples.count / 2 ? trimmed : samples
    }

    nonisolated private static func removeDCOffset(_ samples: inout [Float]) {
        guard !samples.isEmpty else { return }

        let mean = samples.reduce(Float(0), +) / Float(samples.count)
        if abs(mean) < 0.0001 {
            return
        }

        for index in samples.indices {
            samples[index] -= mean
        }
    }

    nonisolated private static func appendLittleEndian<T: FixedWidthInteger>(_ value: T, to data: inout Data) {
        var littleEndianValue = value.littleEndian
        withUnsafeBytes(of: &littleEndianValue) { bytes in
            data.append(bytes.bindMemory(to: UInt8.self))
        }
    }
}
