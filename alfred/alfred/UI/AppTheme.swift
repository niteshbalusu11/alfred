import SwiftUI

enum AppTheme {
    enum Colors {
        // Clean dark palette.
        static let ink = Color(red: 0.03, green: 0.04, blue: 0.06)
        static let charcoal = Color(red: 0.09, green: 0.11, blue: 0.15)
        static let smoke = Color(red: 0.62, green: 0.66, blue: 0.73)
        static let paper = Color(red: 0.95, green: 0.96, blue: 0.98)

        static let background = ink
        static let surface = charcoal
        static let surfaceElevated = Color(red: 0.12, green: 0.15, blue: 0.20)
        static let outline = paper.opacity(0.15)
        static let textPrimary = paper
        static let textSecondary = smoke
        static let accent = Color(red: 0.90, green: 0.92, blue: 0.96)
        static let success = Color(red: 0.62, green: 0.87, blue: 0.72)
        static let warning = Color(red: 0.92, green: 0.79, blue: 0.46)
        static let danger = Color(red: 0.92, green: 0.45, blue: 0.45)
        static let shadow = Color.black.opacity(0.35)
    }

    enum Layout {
        static let screenPadding: CGFloat = 20
        static let sectionSpacing: CGFloat = 16
        static let cardPadding: CGFloat = 16
        static let cardCornerRadius: CGFloat = 18
        static let fieldCornerRadius: CGFloat = 12
        static let buttonCornerRadius: CGFloat = 14
        static let cartoonStrokeWidth: CGFloat = 1
        static let cartoonShadowOffset: CGFloat = 2
        static let softShadowRadius: CGFloat = 12
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
                color: AppTheme.Colors.shadow.opacity(0.55),
                radius: AppTheme.Layout.softShadowRadius,
                x: 0,
                y: AppTheme.Layout.cartoonShadowOffset
            )
    }
}
