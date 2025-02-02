package net.nymtech.nymvpn.ui

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import net.nymtech.nymvpn.data.GatewayRepository
import net.nymtech.nymvpn.data.SettingsRepository
import net.nymtech.nymvpn.service.country.CountryCacheService
import net.nymtech.nymvpn.service.tunnel.TunnelManager
import net.nymtech.nymvpn.ui.common.navigation.NavBarState
import net.nymtech.nymvpn.util.Constants
import net.nymtech.vpn.model.Country
import timber.log.Timber
import javax.inject.Inject

@HiltViewModel
class AppViewModel
@Inject
constructor(
	private val settingsRepository: SettingsRepository,
	gatewayRepository: GatewayRepository,
	private val countryCacheService: CountryCacheService,
	private val tunnelManager: TunnelManager,
) : ViewModel() {

	private val _navBarState = MutableStateFlow(NavBarState())
	val navBarState = _navBarState.asStateFlow()

	val uiState =
		combine(
			settingsRepository.settingsFlow,
			tunnelManager.stateFlow,
			gatewayRepository.gatewayFlow,
		) { settings, manager, gateways ->
			AppUiState(
				settings,
				gateways,
				manager.state,
				manager.backendMessage,
				isMnemonicStored = manager.isMnemonicStored,
				entryCountry = settings.firstHopCountry ?: Country(isLowLatency = true),
				exitCountry = settings.lastHopCountry ?: Country(isDefault = true),
			)
		}.stateIn(
			viewModelScope,
			SharingStarted.WhileSubscribed(Constants.SUBSCRIPTION_TIMEOUT),
			AppUiState(),
		)

	fun setAnalyticsShown() = viewModelScope.launch {
		settingsRepository.setAnalyticsShown(true)
	}

	fun logout() = viewModelScope.launch {
		tunnelManager.removeMnemonic()
	}

	fun onErrorReportingSelected() = viewModelScope.launch {
		settingsRepository.setErrorReporting(!uiState.value.settings.errorReportingEnabled)
	}

	fun onAnalyticsReportingSelected() = viewModelScope.launch {
		settingsRepository.setAnalytics(!uiState.value.settings.analyticsEnabled)
	}

	fun onNavBarStateChange(navBarState: NavBarState) {
		_navBarState.update {
			navBarState
		}
	}

	fun onAppStartup() = viewModelScope.launch {
		launch {
			Timber.d("Updating exit country cache")
			countryCacheService.updateExitCountriesCache().onSuccess {
				Timber.d("Exit countries updated")
			}.onFailure { Timber.w("Failed to get exit countries: ${it.message}") }
		}
		launch {
			Timber.d("Updating entry country cache")
			countryCacheService.updateEntryCountriesCache().onSuccess {
				Timber.d("Entry countries updated")
			}.onFailure { Timber.w("Failed to get entry countries: ${it.message}") }
		}
		launch {
			Timber.d("Updating entry country cache")
			countryCacheService.updateWgCountriesCache().onSuccess {
				Timber.d("Wg countries updated")
			}.onFailure { Timber.w("Failed to get wg countries: ${it.message}") }
		}
	}
}
