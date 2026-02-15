import SwiftUI

enum AppTheme {
    enum Colors {
        static let background = Color(red: 0.05, green: 0.06, blue: 0.08)
        static let surface = Color(red: 0.12, green: 0.13, blue: 0.16)
        static let surfaceElevated = Color(red: 0.17, green: 0.18, blue: 0.22)
        static let outline = Color(red: 0.22, green: 0.23, blue: 0.27)
        static let textPrimary = Color(red: 0.94, green: 0.95, blue: 0.97)
        static let textSecondary = Color(red: 0.70, green: 0.72, blue: 0.78)
        static let accent = Color(red: 0.35, green: 0.74, blue: 0.95)
        static let success = Color(red: 0.32, green: 0.84, blue: 0.56)
        static let warning = Color(red: 0.94, green: 0.74, blue: 0.35)
        static let danger = Color(red: 0.95, green: 0.40, blue: 0.40)
    }

    enum Layout {
        static let screenPadding: CGFloat = 20
        static let sectionSpacing: CGFloat = 16
        static let cardPadding: CGFloat = 16
        static let cardCornerRadius: CGFloat = 16
        static let fieldCornerRadius: CGFloat = 12
        static let buttonCornerRadius: CGFloat = 12
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
                    .stroke(AppTheme.Colors.outline, lineWidth: 1)
            )
    }
}
