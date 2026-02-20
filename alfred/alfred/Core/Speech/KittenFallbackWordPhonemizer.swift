import Foundation

enum KittenFallbackWordPhonemizer {
    nonisolated static func phonemize(
        word: String,
        pronunciationDictionary: [String: String]
    ) -> String? {
        let normalized = word
            .lowercased()
            .replacingOccurrences(of: "'", with: "")
        guard !normalized.isEmpty else { return nil }
        guard normalized.unicodeScalars.allSatisfy({ CharacterSet.letters.contains($0) }) else {
            return nil
        }

        if let segmented = segmentedFallback(
            for: normalized,
            pronunciationDictionary: pronunciationDictionary
        ) {
            return segmented
        }

        // Heuristic G2P is intentionally conservative. Long proper nouns
        // (cities, names, acronyms) are safer to leave as graphemes.
        if normalized.count <= 6 {
            return heuristicPronunciation(for: normalized)
        }

        return nil
    }

    nonisolated private static func segmentedFallback(
        for word: String,
        pronunciationDictionary: [String: String]
    ) -> String? {
        guard word.count >= 4 else { return nil }

        for splitOffset in stride(from: word.count - 2, through: 2, by: -1) {
            let splitIndex = word.index(word.startIndex, offsetBy: splitOffset)
            let prefix = String(word[..<splitIndex])
            let suffix = String(word[splitIndex...])

            if let prefixPhoneme = pronunciationDictionary[prefix],
               let suffixPhoneme = heuristicPronunciation(for: suffix) {
                return prefixPhoneme + suffixPhoneme
            }

            if let suffixPhoneme = pronunciationDictionary[suffix],
               let prefixPhoneme = heuristicPronunciation(for: prefix) {
                return prefixPhoneme + suffixPhoneme
            }
        }

        return nil
    }

    nonisolated private static func heuristicPronunciation(for word: String) -> String? {
        var output = String()
        output.reserveCapacity(word.count * 2)

        var cursor = word.startIndex
        while cursor < word.endIndex {
            let remainder = String(word[cursor...])

            if let (match, phoneme) = terminalCluster(in: remainder, isWordStart: cursor == word.startIndex) {
                output.append(phoneme)
                cursor = word.index(cursor, offsetBy: match.count)
                continue
            }

            if let (match, phoneme) = graphemeCluster(in: remainder) {
                output.append(phoneme)
                cursor = word.index(cursor, offsetBy: match.count)
                continue
            }

            let character = word[cursor]
            let nextCharacter: Character? = {
                let nextIndex = word.index(after: cursor)
                return nextIndex < word.endIndex ? word[nextIndex] : nil
            }()

            if character == "e",
               isTerminalSilentE(in: word, at: cursor) {
                cursor = word.index(after: cursor)
                continue
            }

            if let phoneme = singleLetterPhoneme(for: character, nextCharacter: nextCharacter) {
                output.append(phoneme)
            }

            cursor = word.index(after: cursor)
        }

        guard !output.isEmpty else { return nil }
        return output
    }

    nonisolated private static func terminalCluster(
        in remainder: String,
        isWordStart: Bool
    ) -> (String, String)? {
        guard !isWordStart else { return nil }

        if let phoneme = terminalClusters[remainder] {
            return (remainder, phoneme)
        }

        return nil
    }

    nonisolated private static func graphemeCluster(in remainder: String) -> (String, String)? {
        for (grapheme, phoneme) in graphemeClusters {
            if remainder.hasPrefix(grapheme) {
                return (grapheme, phoneme)
            }
        }
        return nil
    }

    nonisolated private static func isTerminalSilentE(in word: String, at index: String.Index) -> Bool {
        guard index == word.index(before: word.endIndex) else { return false }
        guard word.count >= 3 else { return false }

        let previousIndex = word.index(before: index)
        let previous = word[previousIndex]
        return !vowels.contains(previous)
    }

    nonisolated private static func singleLetterPhoneme(
        for character: Character,
        nextCharacter: Character?
    ) -> String? {
        switch character {
        case "a": return "æ"
        case "b": return "b"
        case "c":
            if let nextCharacter, softeningFollowers.contains(nextCharacter) {
                return "s"
            }
            return "k"
        case "d": return "d"
        case "e": return "ɛ"
        case "f": return "f"
        case "g":
            if let nextCharacter, softeningFollowers.contains(nextCharacter) {
                return "dʒ"
            }
            return "g"
        case "h": return "h"
        case "i": return "ɪ"
        case "j": return "dʒ"
        case "k": return "k"
        case "l": return "l"
        case "m": return "m"
        case "n": return "n"
        case "o": return "oʊ"
        case "p": return "p"
        case "q": return "k"
        case "r": return "ɹ"
        case "s": return "s"
        case "t": return "t"
        case "u": return "ʌ"
        case "v": return "v"
        case "w": return "w"
        case "x": return "ks"
        case "y":
            if nextCharacter == nil {
                return "i"
            }
            return "j"
        case "z": return "z"
        default: return nil
        }
    }

    nonisolated private static let vowels: Set<Character> = Set("aeiouy")
    nonisolated private static let softeningFollowers: Set<Character> = Set("eiy")
    nonisolated private static let terminalClusters: [String: String] = [
        "dhi": "diː",
        "thi": "tiː",
        "khi": "kiː",
        "ghi": "giː",
        "bhi": "biː",
        "phi": "fiː",
        "shi": "ʃiː",
        "chi": "tʃiː",
        "ji": "dʒiː",
        "hi": "iː",
    ]
    nonisolated private static let graphemeClusters: [(String, String)] = [
        ("tion", "ʃən"),
        ("sion", "ʒən"),
        ("ture", "tʃɚ"),
        ("eigh", "eɪ"),
        ("augh", "ɔː"),
        ("ough", "oʊ"),
        ("igh", "aɪ"),
        ("sch", "sk"),
        ("tch", "tʃ"),
        ("ph", "f"),
        ("sh", "ʃ"),
        ("ch", "tʃ"),
        ("th", "θ"),
        ("ng", "ŋ"),
        ("nk", "ŋk"),
        ("qu", "kw"),
        ("ck", "k"),
        ("wh", "w"),
        ("wr", "ɹ"),
        ("kn", "n"),
        ("gn", "n"),
        ("dg", "dʒ"),
        ("ee", "iː"),
        ("ea", "iː"),
        ("oo", "uː"),
        ("ai", "eɪ"),
        ("ay", "eɪ"),
        ("oi", "ɔɪ"),
        ("oy", "ɔɪ"),
        ("ou", "aʊ"),
        ("ow", "aʊ"),
        ("au", "ɔː"),
        ("aw", "ɔː"),
        ("oa", "oʊ"),
        ("ie", "iː"),
        ("ei", "iː"),
        ("ew", "juː"),
    ]
}
