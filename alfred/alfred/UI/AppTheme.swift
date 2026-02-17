import SwiftUI

enum AppTheme {
    enum Colors {
        // Monochrome cartoon palette (four-tone core):
        // ink, charcoal, smoke, paper.
        static let ink = Color(red: 0.05, green: 0.05, blue: 0.05)
        static let charcoal = Color(red: 0.15, green: 0.15, blue: 0.15)
        static let smoke = Color(red: 0.78, green: 0.78, blue: 0.78)
        static let paper = Color(red: 0.97, green: 0.97, blue: 0.97)

        static let background = ink
        static let surface = charcoal
        static let surfaceElevated = Color(red: 0.22, green: 0.22, blue: 0.22)
        static let outline = paper
        static let textPrimary = paper
        static let textSecondary = smoke
        static let accent = paper
        static let success = paper
        static let warning = smoke
        static let danger = Color(red: 0.62, green: 0.62, blue: 0.62)
        static let shadow = Color.black.opacity(0.85)
    }

    enum Layout {
        static let screenPadding: CGFloat = 20
        static let sectionSpacing: CGFloat = 16
        static let cardPadding: CGFloat = 16
        static let cardCornerRadius: CGFloat = 18
        static let fieldCornerRadius: CGFloat = 12
        static let buttonCornerRadius: CGFloat = 14
        static let cartoonStrokeWidth: CGFloat = 2
        static let cartoonShadowOffset: CGFloat = 6
    }
}

extension View {
    func appScreenBackground() -> some View {
        background(AppTheme.Colors.background.ignoresSafeArea())
    }

    func appFieldStyle() -> some View {
        padding(12)
            .foregroundStyle(AppTheme.Colors.textPrimary)
            .background(AppTheme.Colors.surfaceElevated)
            .clipShape(RoundedRectangle(cornerRadius: AppTheme.Layout.fieldCornerRadius, style: .continuous))
            .overlay(
                RoundedRectangle(cornerRadius: AppTheme.Layout.fieldCornerRadius, style: .continuous)
                    .stroke(AppTheme.Colors.outline, lineWidth: AppTheme.Layout.cartoonStrokeWidth)
            )
            .shadow(
                color: AppTheme.Colors.shadow,
                radius: 0,
                x: 0,
                y: AppTheme.Layout.cartoonShadowOffset
            )
    }
}
