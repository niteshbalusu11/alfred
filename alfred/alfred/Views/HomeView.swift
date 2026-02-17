import SwiftUI

struct HomeView: View {
    var body: some View {
        GeometryReader { geometry in
            let horizontalPadding = AppTheme.Layout.screenPadding
            let contentWidth = max(280, min(560, geometry.size.width - (horizontalPadding * 2)))

            ZStack {
                HomeVoiceBackdrop()

                VStack {
                    HStack(spacing: 0) {
                        Spacer(minLength: 0)
                        HomeVoiceTranscriptionSection()
                            .frame(width: contentWidth)
                        Spacer(minLength: 0)
                    }
                    .padding(.top, 18)

                    Spacer(minLength: geometry.safeAreaInsets.bottom + 170)
                }
                .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .top)
            }
        }
        .appScreenBackground()
    }
}

private struct HomeVoiceBackdrop: View {
    var body: some View {
        ZStack {
            Circle()
                .stroke(AppTheme.Colors.smoke.opacity(0.12), lineWidth: 34)
                .frame(width: 290, height: 290)
                .offset(y: -290)

            Circle()
                .fill(AppTheme.Colors.surfaceElevated.opacity(0.3))
                .frame(width: 250, height: 250)
                .offset(y: -150)
                .blur(radius: 8)
        }
    }
}

#Preview {
    HomeView()
}
