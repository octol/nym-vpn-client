import SwiftUI
import AppSettings
import ConnectionManager
import CountriesManager
import Settings

struct HomeFlowCoordinator<Content: View>: View {
    @ObservedObject var state: HomeFlowState

    let isSmallScreen: Bool
    let content: () -> Content

    var body: some View {
        NavigationStack(path: $state.path) {
            ZStack {
                content()
            }
            .navigationDestination(for: HomeLink.self, destination: linkDestination)
        }
    }
}

private extension HomeFlowCoordinator {
    @ViewBuilder private func linkDestination(link: HomeLink) -> some View {
        switch link {
        case .entryHop:
            entryHop()
        case .exitHop:
            exitHop()
        case .settings:
            SettingsView(viewModel: SettingsViewModel(path: $state.path, appSettings: AppSettings.shared))
        }
    }
}

private extension HomeFlowCoordinator {
    private func entryHop() -> some View {
        let viewModel = HopListViewModel(
            type: .entry,
            path: $state.path,
            isSmallScreen: false
        )
        return HopListView(viewModel: viewModel)
    }

    private func exitHop() -> some View {
        let viewModel = HopListViewModel(
            type: .exit,
            path: $state.path,
            isSmallScreen: false
        )
        return HopListView(viewModel: viewModel)
    }
}
