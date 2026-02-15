//
//  alfredApp.swift
//  alfred
//
//  Created by Nitesh Chowdhary Balusu on 2/13/26.
//

import ClerkKit
import ClerkKitUI
import SwiftUI

@main
struct alfredApp: App {
    private let clerk: Clerk
    @StateObject private var model: AppModel

    init() {
        let publishableKey = AppConfiguration.requiredClerkPublishableKey
        let configuredClerk = Clerk.configure(publishableKey: publishableKey)
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
