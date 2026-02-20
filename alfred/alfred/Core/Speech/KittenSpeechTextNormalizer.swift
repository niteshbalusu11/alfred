import Foundation

enum KittenSpeechTextNormalizer {
    private static let integerFormatter: NumberFormatter = {
        let formatter = NumberFormatter()
        formatter.locale = Locale(identifier: "en_US")
        formatter.numberStyle = .spellOut
        return formatter
    }()

    private static let decimalDigitWords: [String] = [
        "zero", "one", "two", "three", "four", "five", "six", "seven", "eight", "nine",
    ]

    private static let timeRegex = try! NSRegularExpression(
        pattern: #"\b(\d{1,2}):(\d{2})(?::(\d{2}))?\s*(am|pm|a\.m\.|p\.m\.)?\s*(UTC|GMT|EST|EDT|CST|CDT|MST|MDT|PST|PDT|BST|IST|CET|CEST|Z)?\b"#,
        options: [.caseInsensitive]
    )

    private static let currencyRegex = try! NSRegularExpression(
        pattern: #"\$\s*(-?[\d,]+(?:\.\d+)?)"#,
        options: []
    )

    private static let percentageRegex = try! NSRegularExpression(
        pattern: #"(-?[\d,]+(?:\.\d+)?)\s*%"#,
        options: []
    )

    private static let decimalRegex = try! NSRegularExpression(
        pattern: #"(?<![A-Za-z0-9])(-?\d+\.\d+)(?![A-Za-z0-9])"#,
        options: []
    )

    private static let groupedIntegerRegex = try! NSRegularExpression(
        pattern: #"(?<![A-Za-z0-9])(-?\d{1,3}(?:,\d{3})+)(?![A-Za-z0-9])"#,
        options: []
    )

    private static let integerRegex = try! NSRegularExpression(
        pattern: #"(?<![A-Za-z0-9])(-?\d+)(?![A-Za-z0-9])"#,
        options: []
    )

    static func normalize(_ text: String) -> String {
        var normalized = text.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !normalized.isEmpty else { return "" }

        normalized = normalizeMarkdown(in: normalized)
        normalized = normalizeSymbols(in: normalized)
        normalized = replacingMatches(in: normalized, using: timeRegex, replacement: replaceTime)
        normalized = replacingMatches(in: normalized, using: currencyRegex, replacement: replaceCurrency)
        normalized = replacingMatches(in: normalized, using: percentageRegex, replacement: replacePercentage)
        normalized = replacingMatches(in: normalized, using: decimalRegex, replacement: replaceDecimal)
        normalized = replacingMatches(in: normalized, using: groupedIntegerRegex, replacement: replaceInteger)
        normalized = replacingMatches(in: normalized, using: integerRegex, replacement: replaceInteger)

        normalized = normalized
            .replacingOccurrences(of: #"\s+"#, with: " ", options: .regularExpression)
            .trimmingCharacters(in: .whitespacesAndNewlines)

        return normalized
    }

    private static func normalizeMarkdown(in text: String) -> String {
        var normalized = text

        normalized = normalized.replacingOccurrences(
            of: #"```[\s\S]*?```"#,
            with: " ",
            options: .regularExpression
        )
        normalized = normalized.replacingOccurrences(
            of: #"`([^`]*)`"#,
            with: "$1",
            options: .regularExpression
        )
        normalized = normalized.replacingOccurrences(
            of: #"\[([^\]]+)\]\(([^)]+)\)"#,
            with: "$1",
            options: .regularExpression
        )
        normalized = normalized.replacingOccurrences(
            of: #"(?m)^\s*[-*]\s+"#,
            with: "",
            options: .regularExpression
        )

        return normalized
    }

    private static func normalizeSymbols(in text: String) -> String {
        text
            .replacingOccurrences(of: "&", with: " and ")
            .replacingOccurrences(of: "—", with: ", ")
            .replacingOccurrences(of: "–", with: " to ")
            .replacingOccurrences(of: "…", with: "...")
    }

    private static func replaceTime(_ text: String, _ result: NSTextCheckingResult) -> String {
        guard let hour = captureInt(1, from: text, result: result),
              let minute = captureInt(2, from: text, result: result)
        else {
            return captureText(from: text, result: result)
        }

        let second = captureInt(3, from: text, result: result)
        let ampmRaw = captureString(4, from: text, result: result)
        let timezoneRaw = captureString(5, from: text, result: result)

        let spokenTime: String
        if let ampmRaw {
            let ampm = normalizeAMPM(ampmRaw)
            let hour12 = hour % 12 == 0 ? 12 : hour % 12
            let hourWords = integerWords(hour12)
            if let second {
                if minute == 0 && second == 0 {
                    spokenTime = "\(hourWords) \(ampm)"
                } else {
                    let minuteSpoken = minuteWords(minute)
                    let secondSpoken = minuteWords(second)
                    spokenTime = "\(hourWords) \(minuteSpoken) and \(secondSpoken) seconds \(ampm)"
                }
            } else if minute == 0 {
                spokenTime = "\(hourWords) \(ampm)"
            } else {
                spokenTime = "\(hourWords) \(minuteWords(minute)) \(ampm)"
            }
        } else {
            let hourWords = integerWords(hour)
            if let second {
                if minute == 0 && second == 0 {
                    spokenTime = "\(hourWords) o'clock"
                } else {
                    spokenTime = "\(hourWords) \(minuteWords(minute)) and \(minuteWords(second)) seconds"
                }
            } else if minute == 0 {
                spokenTime = "\(hourWords) hundred"
            } else {
                spokenTime = "\(hourWords) \(minuteWords(minute))"
            }
        }

        if let timezoneRaw {
            return "\(spokenTime) \(spokenInitialism(timezoneRaw))"
        }
        return spokenTime
    }

    private static func replaceCurrency(_ text: String, _ result: NSTextCheckingResult) -> String {
        guard let value = captureString(1, from: text, result: result) else {
            return captureText(from: text, result: result)
        }

        let normalizedValue = value.replacingOccurrences(of: ",", with: "")
        if normalizedValue.contains(".") {
            return "\(decimalWords(normalizedValue)) dollars"
        }

        guard let integerValue = Int(normalizedValue) else {
            return captureText(from: text, result: result)
        }
        let unit = abs(integerValue) == 1 ? "dollar" : "dollars"
        return "\(integerWords(integerValue)) \(unit)"
    }

    private static func replacePercentage(_ text: String, _ result: NSTextCheckingResult) -> String {
        guard let value = captureString(1, from: text, result: result) else {
            return captureText(from: text, result: result)
        }

        let normalizedValue = value.replacingOccurrences(of: ",", with: "")
        if normalizedValue.contains(".") {
            return "\(decimalWords(normalizedValue)) percent"
        }

        guard let integerValue = Int(normalizedValue) else {
            return captureText(from: text, result: result)
        }
        return "\(integerWords(integerValue)) percent"
    }

    private static func replaceDecimal(_ text: String, _ result: NSTextCheckingResult) -> String {
        guard let value = captureString(1, from: text, result: result) else {
            return captureText(from: text, result: result)
        }
        return decimalWords(value)
    }

    private static func replaceInteger(_ text: String, _ result: NSTextCheckingResult) -> String {
        guard let value = captureString(1, from: text, result: result) else {
            return captureText(from: text, result: result)
        }

        let normalizedValue = value.replacingOccurrences(of: ",", with: "")
        guard let integerValue = Int(normalizedValue) else {
            return captureText(from: text, result: result)
        }
        return integerWords(integerValue)
    }

    private static func decimalWords(_ raw: String) -> String {
        let negative = raw.hasPrefix("-")
        let unsigned = negative ? String(raw.dropFirst()) : raw

        let components = unsigned.split(separator: ".", omittingEmptySubsequences: false)
        guard components.count == 2 else {
            return raw
        }

        let integerPart = String(components[0])
        let decimalPart = String(components[1])

        guard let integerValue = Int(integerPart), !decimalPart.isEmpty else {
            return raw
        }

        let decimalDigits = decimalPart.compactMap { digit -> String? in
            guard let scalar = digit.wholeNumberValue,
                  scalar >= 0,
                  scalar < decimalDigitWords.count
            else {
                return nil
            }
            return decimalDigitWords[scalar]
        }

        guard decimalDigits.count == decimalPart.count else {
            return raw
        }

        let prefix = negative ? "negative " : ""
        return "\(prefix)\(integerWords(integerValue)) point \(decimalDigits.joined(separator: " "))"
    }

    private static func integerWords(_ value: Int) -> String {
        integerFormatter.string(from: NSNumber(value: value)) ?? "\(value)"
    }

    private static func minuteWords(_ value: Int) -> String {
        if value < 10 {
            return "oh \(integerWords(value))"
        }
        return integerWords(value)
    }

    private static func normalizeAMPM(_ raw: String) -> String {
        let compact = raw.lowercased().replacingOccurrences(of: ".", with: "")
        return compact == "pm" ? "p m" : "a m"
    }

    private static func spokenInitialism(_ raw: String) -> String {
        let upper = raw.uppercased()
        if upper == "Z" {
            return "U T C"
        }

        return upper
            .filter(\.isLetter)
            .map { String($0) }
            .joined(separator: " ")
    }

    private static func replacingMatches(
        in text: String,
        using regex: NSRegularExpression,
        replacement: (String, NSTextCheckingResult) -> String
    ) -> String {
        let nsRange = NSRange(text.startIndex..., in: text)
        let matches = regex.matches(in: text, options: [], range: nsRange)
        guard !matches.isEmpty else { return text }

        var output = text
        for match in matches.reversed() {
            guard let range = Range(match.range, in: output) else { continue }
            let replacementValue = replacement(output, match)
            output.replaceSubrange(range, with: replacementValue)
        }
        return output
    }

    private static func captureString(_ index: Int, from text: String, result: NSTextCheckingResult) -> String? {
        guard index < result.numberOfRanges else { return nil }
        let nsRange = result.range(at: index)
        guard nsRange.location != NSNotFound,
              let range = Range(nsRange, in: text)
        else {
            return nil
        }
        return String(text[range])
    }

    private static func captureInt(_ index: Int, from text: String, result: NSTextCheckingResult) -> Int? {
        guard let value = captureString(index, from: text, result: result) else { return nil }
        return Int(value)
    }

    private static func captureText(from text: String, result: NSTextCheckingResult) -> String {
        guard let range = Range(result.range, in: text) else { return "" }
        return String(text[range])
    }
}
