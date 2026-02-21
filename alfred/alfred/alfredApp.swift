//
//  alfredApp.swift
//  alfred
//
//  Created by Nitesh Chowdhary Balusu on 2/13/26.
//

import AlfredAPIClient
import ClerkKit
import ClerkKitUI
import IQKeyboardManagerSwift
import IQKeyboardToolbarManager
import SwiftUI

@main
struct alfredApp: App {
    private let clerk: Clerk
    @UIApplicationDelegateAdaptor(PushAppDelegate.self) private var pushAppDelegate
    @StateObject private var model: AppModel

    @MainActor
    init() {
        let publishableKey = AppConfiguration.requiredClerkPublishableKey
        let configuredClerk = Clerk.configure(publishableKey: publishableKey)
        IQKeyboardManager.shared.isEnabled = true
        IQKeyboardManager.shared.keyboardDistance = 12
        IQKeyboardManager.shared.resignOnTouchOutside = true
        IQKeyboardToolbarManager.shared.isEnabled = false
        do {
            _ = try AutomationNotificationCrypto.registrationMaterial()
        } catch {
            AppLogger.warning("Notification key material bootstrap failed.")
        }
        self.clerk = configuredClerk
        _model = StateObject(wrappedValue: AppModel(clerk: configuredClerk))
    }

    var body: some Scene {
        WindowGroup {
            ContentView(model: model)
                .prefetchClerkImages()
                .environment(clerk)
                .preferredColorScheme(.dark)
                .tint(AppTheme.Colors.accent)
        }
    }
}
