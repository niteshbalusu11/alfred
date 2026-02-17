import SwiftUI

struct HomeVoiceTranscriptionSection: View {
    @StateObject private var transcriptionController = VoiceTranscriptionController()

    private var statusBadge: (title: String, style: AppStatusBadge.Style) {
        switch transcriptionController.status {
        case .idle:
            return ("Ready", .neutral)
        case .requestingPermissions:
            return ("Checking access", .warning)
        case .listening:
            return ("Listening", .success)
        case .permissionDenied:
            return ("Permission needed", .danger)
        case .restricted:
            return ("Restricted", .danger)
        case .unavailable:
            return ("Unavailable", .danger)
        case .failed:
            return ("Error", .danger)
        }
    }

    private var statusMessage: String {
        switch transcriptionController.status {
        case .idle:
            return "Tap the mic and start talking. Transcription stays on-device."
        case .requestingPermissions:
            return "Waiting for speech and microphone authorization."
        case .listening:
            return "Listening now. Your words appear in real time."
        case .permissionDenied:
            return "Enable Microphone and Speech Recognition in iOS Settings."
        case .restricted:
            return "Speech recognition is restricted on this device."
        case .unavailable:
            return "Speech recognition is not available for your current locale."
        case .failed(let message):
            return message
        }
    }

    var body: some View {
        VStack(spacing: 18) {
            HStack {
                AppStatusBadge(title: statusBadge.title, style: statusBadge.style)
            }
            .frame(maxWidth: .infinity, alignment: .center)

            Text("Voice Input")
                .font(.system(size: 30, weight: .black, design: .rounded))
                .foregroundStyle(AppTheme.Colors.textPrimary)
                .frame(maxWidth: .infinity, alignment: .center)

            waveformView

            Text(statusMessage)
                .font(.footnote)
                .foregroundStyle(AppTheme.Colors.textSecondary)
                .multilineTextAlignment(.center)
                .frame(maxWidth: .infinity, alignment: .center)
                .padding(.horizontal, 24)
                .fixedSize(horizontal: false, vertical: true)

            transcriptView
            controlButtons
        }
        .frame(maxWidth: .infinity, alignment: .center)
        .onDisappear {
            transcriptionController.stopRecording()
        }
    }

    private var waveformView: some View {
        ZStack {
            Ellipse()
                .fill(AppTheme.Colors.surfaceElevated.opacity(0.44))
                .frame(height: 56)
                .blur(radius: 12)

            LiveWaveformView(isActive: transcriptionController.isListening)
                .frame(height: 116)
        }
        .frame(maxWidth: .infinity)
        .clipped()
    }

    private var transcriptView: some View {
        Group {
            if transcriptionController.transcript.isEmpty {
                Text("Transcript appears here while you speak.")
                    .foregroundStyle(AppTheme.Colors.textSecondary)
                    .frame(maxWidth: .infinity, alignment: .center)
            } else {
                ScrollView(.vertical, showsIndicators: false) {
                    Text(transcriptionController.transcript)
                        .multilineTextAlignment(.center)
                        .frame(maxWidth: .infinity, alignment: .center)
                }
            }
        }
        .font(.subheadline.weight(.semibold))
        .multilineTextAlignment(.center)
        .frame(maxWidth: .infinity, minHeight: 96, maxHeight: 170, alignment: .top)
        .padding(.horizontal, 12)
        .shadow(color: AppTheme.Colors.shadow.opacity(0.35), radius: 0, x: 0, y: 2)
    }

    private var controlButtons: some View {
        VStack(spacing: 12) {
            HStack(spacing: 20) {
                Button {
                    Task {
                        await transcriptionController.toggleRecording()
                    }
                } label: {
                    MicControlButtonGlyph(
                        isListening: transcriptionController.isListening,
                        isDisabled: transcriptionController.isRequestingPermissions
                    )
                }
                .buttonStyle(.plain)
                .disabled(transcriptionController.isRequestingPermissions)
                .accessibilityLabel(transcriptionController.isListening ? "Stop recording" : "Start recording")

                Button {
                    transcriptionController.clearTranscript()
                } label: {
                    CircleActionButtonGlyph(systemName: "xmark", label: "Clear")
                }
                .buttonStyle(.plain)
                .disabled(transcriptionController.transcript.isEmpty)
            }
            .frame(maxWidth: .infinity, alignment: .center)

            Text(transcriptionController.isListening ? "Listening…" : "Tap mic to start")
                .font(.subheadline.weight(.semibold))
                .foregroundStyle(AppTheme.Colors.textSecondary)
                .frame(maxWidth: .infinity, alignment: .center)
        }
        .padding(.top, 90)
    }
}

private struct LiveWaveformView: View {
    let isActive: Bool

    var body: some View {
        TimelineView(.animation(minimumInterval: 1.0 / 30.0, paused: !isActive)) { timeline in
            Canvas { context, size in
                let drawableWidth = max(size.width, 1)
                let time = timeline.date.timeIntervalSinceReferenceDate
                let baselineY = size.height / 2
                let activeAmplitude = size.height * 0.28
                let idleAmplitude = size.height * 0.035
                let baseAmplitude = isActive ? activeAmplitude : idleAmplitude
                let frequency = isActive ? 2.4 : 1.5

                context.clip(to: Path(CGRect(origin: .zero, size: size)))

                for layer in 0..<4 {
                    let phaseOffset = Double(layer) * 0.95
                    let amplitudeScale = 1.0 - (Double(layer) * 0.18)
                    let phase = (time * 4.2) + phaseOffset
                    let layerOpacity = isActive ? (0.75 - (Double(layer) * 0.14)) : 0.25
                    let colorPair = Self.waveColorPairs[layer % Self.waveColorPairs.count]
                    var path = Path()
                    var didMove = false

                    for x in stride(from: 0.0, through: drawableWidth, by: 3.0) {
                        let normalizedX = Double(x / drawableWidth)
                        let envelope = pow(sin(.pi * normalizedX), 1.35)
                        let oscillation = sin((normalizedX * .pi * 2.0 * frequency) + phase)
                        let offsetY = oscillation * Double(baseAmplitude) * envelope * amplitudeScale
                        let point = CGPoint(x: x, y: baselineY + CGFloat(offsetY))

                        if !didMove {
                            path.move(to: point)
                            didMove = true
                        } else {
                            path.addLine(to: point)
                        }
                    }

                    context.stroke(
                        path,
                        with: .linearGradient(
                            Gradient(colors: [
                                colorPair.0.opacity(layerOpacity),
                                colorPair.1.opacity(layerOpacity)
                            ]),
                            startPoint: CGPoint(x: 0, y: baselineY),
                            endPoint: CGPoint(x: drawableWidth, y: baselineY)
                        ),
                        style: StrokeStyle(lineWidth: CGFloat(3.2 - (Double(layer) * 0.5)), lineCap: .round, lineJoin: .round)
                    )
                }
            }
            .overlay {
                Capsule(style: .continuous)
                    .fill(
                        LinearGradient(
                            colors: [
                                Color(red: 0.24, green: 0.86, blue: 0.99).opacity(isActive ? 0.35 : 0.12),
                                Color(red: 0.29, green: 0.97, blue: 0.68).opacity(isActive ? 0.35 : 0.12)
                            ],
                            startPoint: .leading,
                            endPoint: .trailing
                        )
                    )
                    .frame(height: 2)
            }
        }
    }

    private static let waveColorPairs: [(Color, Color)] = [
        (Color(red: 0.23, green: 0.86, blue: 0.99), Color(red: 0.14, green: 0.53, blue: 0.99)),
        (Color(red: 0.17, green: 0.72, blue: 1.00), Color(red: 0.30, green: 0.97, blue: 0.67)),
        (Color(red: 0.31, green: 0.95, blue: 0.73), Color(red: 0.99, green: 0.80, blue: 0.30)),
        (Color(red: 0.17, green: 0.56, blue: 1.00), Color(red: 0.97, green: 0.46, blue: 0.30))
    ]
}

private struct MicControlButtonGlyph: View {
    let isListening: Bool
    let isDisabled: Bool

    var body: some View {
        ZStack {
            if isListening {
                ListeningPulseRing()
                    .frame(width: 96, height: 96)
            }

            Circle()
                .fill(AppTheme.Colors.accent.opacity(isDisabled ? 0.45 : 1.0))
                .frame(width: 84, height: 84)
                .overlay(
                    Circle()
                        .stroke(AppTheme.Colors.ink, lineWidth: AppTheme.Layout.cartoonStrokeWidth)
                )
                .shadow(
                    color: AppTheme.Colors.shadow.opacity(0.88),
                    radius: 0,
                    x: 0,
                    y: AppTheme.Layout.cartoonShadowOffset
                )
                .overlay(
                    Image(systemName: isListening ? "stop.fill" : "mic.fill")
                        .font(.system(size: 29, weight: .black))
                        .foregroundStyle(AppTheme.Colors.ink)
                )
        }
    }
}

private struct CircleActionButtonGlyph: View {
    let systemName: String
    let label: String

    var body: some View {
        VStack(spacing: 6) {
            Circle()
                .fill(AppTheme.Colors.surfaceElevated)
                .frame(width: 56, height: 56)
                .overlay(
                    Circle()
                        .stroke(AppTheme.Colors.outline, lineWidth: AppTheme.Layout.cartoonStrokeWidth)
                )
                .shadow(
                    color: AppTheme.Colors.shadow.opacity(0.7),
                    radius: 0,
                    x: 0,
                    y: AppTheme.Layout.cartoonShadowOffset
                )
                .overlay(
                    Image(systemName: systemName)
                        .font(.system(size: 18, weight: .bold))
                        .foregroundStyle(AppTheme.Colors.textPrimary)
                )

            Text(label)
                .font(.caption.weight(.bold))
                .foregroundStyle(AppTheme.Colors.textSecondary)
        }
    }
}

private struct ListeningPulseRing: View {
    var body: some View {
        TimelineView(.animation(minimumInterval: 1.0 / 30.0, paused: false)) { timeline in
            let cycleDuration = 1.1
            let elapsed = timeline.date.timeIntervalSinceReferenceDate.truncatingRemainder(dividingBy: cycleDuration)
            let progress = elapsed / cycleDuration
            let scale = 1.0 + (progress * 0.44)
            let opacity = 1.0 - progress

            Circle()
                .stroke(AppTheme.Colors.paper.opacity(opacity * 0.85), lineWidth: 2)
                .scaleEffect(scale)
        }
    }
}

#Preview {
    HomeVoiceTranscriptionSection()
        .padding()
        .appScreenBackground()
}
