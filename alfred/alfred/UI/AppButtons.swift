import SwiftUI

struct AppPrimaryButtonStyle: ButtonStyle {
    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .font(.headline.weight(.black))
            .foregroundStyle(AppTheme.Colors.ink)
            .frame(maxWidth: .infinity, minHeight: 44)
            .padding(.horizontal, 12)
            .background(
                AppTheme.Colors.accent
                    .opacity(configuration.isPressed ? 0.85 : 1.0)
            )
            .clipShape(RoundedRectangle(cornerRadius: AppTheme.Layout.buttonCornerRadius, style: .continuous))
            .overlay(
                RoundedRectangle(cornerRadius: AppTheme.Layout.buttonCornerRadius, style: .continuous)
                    .stroke(AppTheme.Colors.ink, lineWidth: AppTheme.Layout.cartoonStrokeWidth)
            )
            .shadow(
                color: AppTheme.Colors.shadow.opacity(configuration.isPressed ? 0.6 : 0.9),
                radius: 0,
                x: 0,
                y: configuration.isPressed ? 2 : AppTheme.Layout.cartoonShadowOffset
            )
    }
}

struct AppSecondaryButtonStyle: ButtonStyle {
    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .font(.headline.weight(.black))
            .foregroundStyle(AppTheme.Colors.textPrimary)
            .frame(maxWidth: .infinity, minHeight: 44)
            .padding(.horizontal, 12)
            .background(
                AppTheme.Colors.surfaceElevated
                    .opacity(configuration.isPressed ? 0.8 : 1.0)
            )
            .clipShape(RoundedRectangle(cornerRadius: AppTheme.Layout.buttonCornerRadius, style: .continuous))
            .overlay(
                RoundedRectangle(cornerRadius: AppTheme.Layout.buttonCornerRadius, style: .continuous)
                    .stroke(AppTheme.Colors.outline, lineWidth: AppTheme.Layout.cartoonStrokeWidth)
            )
            .shadow(
                color: AppTheme.Colors.shadow.opacity(configuration.isPressed ? 0.45 : 0.8),
                radius: 0,
                x: 0,
                y: configuration.isPressed ? 2 : AppTheme.Layout.cartoonShadowOffset
            )
    }
}

extension ButtonStyle where Self == AppPrimaryButtonStyle {
    static var appPrimary: AppPrimaryButtonStyle { AppPrimaryButtonStyle() }
}

extension ButtonStyle where Self == AppSecondaryButtonStyle {
    static var appSecondary: AppSecondaryButtonStyle { AppSecondaryButtonStyle() }
}
