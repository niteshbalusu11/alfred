import Foundation

enum KittenEnglishPhonemizerError: Error {
    case missingDictionary
    case invalidDictionary
}

actor KittenEnglishPhonemizer {
    private let languageCode = "en_us"
    private let dictionaryResourceName = "open_phonemizer_en_us_dict"

    private var pronunciationDictionary: [String: String]?
    private let numberFormatter: NumberFormatter = {
        let formatter = NumberFormatter()
        formatter.locale = Locale(identifier: "en_US")
        formatter.numberStyle = .spellOut
        return formatter
    }()

    init(pronunciationDictionary: [String: String]? = nil) {
        self.pronunciationDictionary = pronunciationDictionary
    }

    func phonemize(_ text: String) throws -> String {
        let dictionary = try loadDictionary()
        return Self.phonemize(
            text,
            pronunciationDictionary: dictionary,
            numberFormatter: numberFormatter
        )
    }

    private func loadDictionary() throws -> [String: String] {
        if let pronunciationDictionary {
            return pronunciationDictionary
        }

        guard let dictionaryURL = bundledDictionaryURL() else {
            throw KittenEnglishPhonemizerError.missingDictionary
        }

        let data = try Data(contentsOf: dictionaryURL)
        let rootObject = try JSONSerialization.jsonObject(with: data)

        let dictionary: [String: String]
        if let languageRoot = rootObject as? [String: [String: String]],
           let languageDictionary = languageRoot[languageCode] {
            dictionary = languageDictionary
        } else if let directDictionary = rootObject as? [String: String] {
            dictionary = directDictionary
        } else {
            throw KittenEnglishPhonemizerError.invalidDictionary
        }

        pronunciationDictionary = dictionary
        return dictionary
    }

    nonisolated private func bundledDictionaryURL() -> URL? {
        if let directURL = Bundle.main.url(
            forResource: dictionaryResourceName,
            withExtension: "json"
        ) {
            return directURL
        }

        return Bundle.main.url(
            forResource: dictionaryResourceName,
            withExtension: "json",
            subdirectory: "KittenTTS"
        )
    }

    nonisolated private static func phonemize(
        _ text: String,
        pronunciationDictionary: [String: String],
        numberFormatter: NumberFormatter
    ) -> String {
        guard !text.isEmpty else { return text }

        let fullRange = NSRange(location: 0, length: text.utf16.count)
        let matches = tokenRegex.matches(in: text, range: fullRange)
        guard !matches.isEmpty else { return text }

        var output = String()
        output.reserveCapacity(text.count * 2)

        for match in matches {
            guard let range = Range(match.range, in: text) else { continue }
            let token = String(text[range])
            if token.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
                output.append(token)
                continue
            }

            if let phonemeToken = phonemeToken(
                from: token,
                pronunciationDictionary: pronunciationDictionary,
                numberFormatter: numberFormatter
            ) {
                output.append(phonemeToken)
            } else {
                output.append(normalizedPunctuation(token))
            }
        }

        return output
    }

    nonisolated private static func phonemeToken(
        from token: String,
        pronunciationDictionary: [String: String],
        numberFormatter: NumberFormatter
    ) -> String? {
        if isNumberToken(token),
           let numericValue = Int(token),
           let spokenValue = numberFormatter.string(from: NSNumber(value: numericValue)) {
            return phonemize(
                spokenValue,
                pronunciationDictionary: pronunciationDictionary,
                numberFormatter: numberFormatter
            )
        }

        if let initialism = phonemeForInitialism(token) {
            return initialism
        }

        guard isWordToken(token) else {
            return nil
        }

        let normalizedWord = token.lowercased()
        if let custom = customPronunciations[normalizedWord] {
            return custom
        }

        if let direct = pronunciationDictionary[normalizedWord] {
            return direct
        }

        if normalizedWord.hasSuffix("'s") {
            let baseWord = String(normalizedWord.dropLast(2))
            if let base = pronunciationDictionary[baseWord] {
                return base + "z"
            }
        }

        if let fallback = KittenFallbackWordPhonemizer.phonemize(
            word: normalizedWord,
            pronunciationDictionary: pronunciationDictionary
        ) {
            return fallback
        }

        return token
    }

    nonisolated private static func phonemeForInitialism(_ token: String) -> String? {
        if let dottedInitialism = phonemeForDottedInitialism(token) {
            return dottedInitialism
        }

        let normalized = token.lowercased()
        if let knownInitialism = knownInitialisms[normalized] {
            return knownInitialism
        }

        if token.count == 1,
           let character = token.first,
           let phoneme = letterPhonemes[character.lowercased().first ?? character] {
            return phoneme
        }

        guard token.count <= 5 else { return nil }
        guard token.unicodeScalars.allSatisfy({ CharacterSet.uppercaseLetters.contains($0) }) else {
            return nil
        }

        let phonemes = token.compactMap { letterPhonemes[$0.lowercased().first ?? $0] }
        guard phonemes.count == token.count else { return nil }
        return phonemes.joined(separator: " ")
    }

    nonisolated private static func phonemeForDottedInitialism(_ token: String) -> String? {
        guard token.contains(".") else { return nil }

        let letters = token.filter { $0 != "." }
        guard !letters.isEmpty else { return nil }
        guard letters.unicodeScalars.allSatisfy({ CharacterSet.letters.contains($0) }) else {
            return nil
        }

        let phonemes = letters.compactMap { letterPhonemes[$0.lowercased().first ?? $0] }
        guard phonemes.count == letters.count else { return nil }
        return phonemes.joined(separator: " ")
    }

    nonisolated private static func normalizedPunctuation(_ token: String) -> String {
        var normalized = String()
        normalized.reserveCapacity(token.count)

        for character in token {
            if allowedPunctuation.contains(character) {
                normalized.append(character)
            } else {
                normalized.append(",")
            }
        }

        return normalized
    }

    nonisolated private static func isWordToken(_ token: String) -> Bool {
        token.unicodeScalars.allSatisfy { scalar in
            CharacterSet.letters.contains(scalar) || scalar == "'"
        }
    }

    nonisolated private static func isNumberToken(_ token: String) -> Bool {
        token.unicodeScalars.allSatisfy { CharacterSet.decimalDigits.contains($0) }
    }

    nonisolated private static let customPronunciations: [String: String] = [
        "i'm": "ˈaɪ ˈæm",
        "washington": "wˈɑːʃɪŋtən",
        "columbia": "kəlˈʌmbiə",
        "delhi": "dˈɛli",
        "mumbai": "mʊmbˈaɪ",
        "chennai": "tʃɛnˈaɪ",
        "kolkata": "koʊlkˈɑːtə",
        "philadelphia": "fɪlədˈɛlfiə",
        "angeles": "ˈændʒələs",
        "francisco": "frənsˈɪskoʊ",
        "seattle": "siˈætəl",
        "houston": "hjˈuːstən",
        "chicago": "ʃɪkˈɑːgoʊ",
        "miami": "maɪˈæmi",
        "atlanta": "ætlˈæntə",
        "dallas": "dˈæləs",
        "phoenix": "fˈiːnɪks",
    ]
    nonisolated private static let knownInitialisms: [String: String] = [
        "la": "ˈɛl ˈeɪ",
        "sf": "ˈɛs ˈɛf",
        "sfo": "ˈɛs ˈɛf ˈoʊ",
        "nyc": "ˈɛn wˈaɪ sˈiː",
        "usa": "jˈuː ˈɛs ˈeɪ",
        "us": "jˈuː ˈɛs",
        "dc": "dˈiː sˈiː",
    ]
    nonisolated private static let letterPhonemes: [Character: String] = [
        "a": "ˈeɪ",
        "b": "bˈiː",
        "c": "sˈiː",
        "d": "dˈiː",
        "e": "ˈiː",
        "f": "ˈɛf",
        "g": "dʒˈiː",
        "h": "ˈeɪtʃ",
        "i": "ˈaɪ",
        "j": "dʒˈeɪ",
        "k": "kˈeɪ",
        "l": "ˈɛl",
        "m": "ˈɛm",
        "n": "ˈɛn",
        "o": "ˈoʊ",
        "p": "pˈiː",
        "q": "kjˈuː",
        "r": "ˈɑːɹ",
        "s": "ˈɛs",
        "t": "tˈiː",
        "u": "jˈuː",
        "v": "vˈiː",
        "w": "dˈʌbəljuː",
        "x": "ˈɛks",
        "y": "wˈaɪ",
        "z": "zˈiː",
    ]
    nonisolated private static let allowedPunctuation: Set<Character> = Set(".,!?;:-")
    nonisolated private static let tokenRegex = try! NSRegularExpression(
        pattern: #"[A-Za-z](?:\.[A-Za-z])+\.?|[A-Za-z]+(?:'[A-Za-z]+)?|[0-9]+|\s+|[^A-Za-z0-9\s]+"#
    )
}
