package net.nymtech.nymvpn.ui.screens.main

import android.app.Activity.RESULT_OK
import android.net.VpnService
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.asPaddingValues
import androidx.compose.foundation.layout.defaultMinSize
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.systemBars
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.outlined.Info
import androidx.compose.material.icons.outlined.Settings
import androidx.compose.material.icons.outlined.Speed
import androidx.compose.material.icons.outlined.VisibilityOff
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.ripple
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.res.vectorResource
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import kotlinx.coroutines.launch
import net.nymtech.nymvpn.R
import net.nymtech.nymvpn.ui.AppUiState
import net.nymtech.nymvpn.ui.AppViewModel
import net.nymtech.nymvpn.ui.Route
import net.nymtech.nymvpn.ui.common.Modal
import net.nymtech.nymvpn.ui.common.animations.SpinningIcon
import net.nymtech.nymvpn.ui.common.buttons.IconSurfaceButton
import net.nymtech.nymvpn.ui.common.buttons.MainStyledButton
import net.nymtech.nymvpn.ui.common.functions.countryIcon
import net.nymtech.nymvpn.ui.common.labels.GroupLabel
import net.nymtech.nymvpn.ui.common.labels.StatusInfoLabel
import net.nymtech.nymvpn.ui.common.navigation.LocalNavController
import net.nymtech.nymvpn.ui.common.navigation.MainTitle
import net.nymtech.nymvpn.ui.common.navigation.NavBarState
import net.nymtech.nymvpn.ui.common.navigation.NavIcon
import net.nymtech.nymvpn.ui.common.snackbar.SnackbarController
import net.nymtech.nymvpn.ui.common.textbox.CustomTextField
import net.nymtech.nymvpn.ui.model.ConnectionState
import net.nymtech.nymvpn.ui.model.StateMessage
import net.nymtech.nymvpn.ui.screens.permission.Permission
import net.nymtech.nymvpn.ui.theme.CustomColors
import net.nymtech.nymvpn.ui.theme.CustomTypography
import net.nymtech.nymvpn.ui.theme.Theme
import net.nymtech.nymvpn.ui.theme.iconSize
import net.nymtech.nymvpn.util.Constants
import net.nymtech.nymvpn.util.extensions.buildCountryNameString
import net.nymtech.nymvpn.util.extensions.goFromRoot
import net.nymtech.nymvpn.util.extensions.openWebUrl
import net.nymtech.nymvpn.util.extensions.scaledHeight
import net.nymtech.nymvpn.util.extensions.scaledWidth
import net.nymtech.nymvpn.util.extensions.toUserMessage
import net.nymtech.vpn.backend.Tunnel

@Composable
fun MainScreen(appViewModel: AppViewModel, appUiState: AppUiState, autoStart: Boolean, viewModel: MainViewModel = hiltViewModel()) {
	val uiState by viewModel.uiState.collectAsStateWithLifecycle()
	val context = LocalContext.current
	val snackbar = SnackbarController.current
	val scope = rememberCoroutineScope()
	val padding = WindowInsets.systemBars.asPaddingValues()
	val navController = LocalNavController.current

	var didAutoStart by remember { mutableStateOf(false) }
	var showDialog by remember { mutableStateOf(false) }

	LaunchedEffect(Unit) {
		appViewModel.onNavBarStateChange(
			NavBarState(
				title = { MainTitle(appUiState.settings.theme ?: Theme.default()) },
				trailing = {
					NavIcon(Icons.Outlined.Settings) {
						navController.goFromRoot(Route.Settings)
					}
				},
			),
		)
	}

	val vpnActivityResultState =
		rememberLauncherForActivityResult(
			ActivityResultContracts.StartActivityForResult(),
			onResult = {
				val accepted = (it.resultCode == RESULT_OK)
				if (!accepted) {
					navController.goFromRoot(Route.Permission(Permission.VPN))
				} else {
					viewModel.onConnect()
				}
			},
		)

	fun onConnectPressed() {
		val intent = VpnService.prepare(context)
		if (intent != null) {
			vpnActivityResultState.launch(
				intent,
			)
		} else {
			viewModel.onConnect()
		}
	}

	if (autoStart && !didAutoStart) {
		LaunchedEffect(Unit) {
			didAutoStart = true
			onConnectPressed()
		}
	}

	Modal(show = showDialog, onDismiss = { showDialog = false }, title = {
		Text(
			text = stringResource(R.string.mode_selection),
			color = MaterialTheme.colorScheme.onSurface,
			style = CustomTypography.labelHuge,
		)
	}, text = {
		ModeModalBody(
			onClick = {
				context.openWebUrl(context.getString(R.string.mode_support_link))
			},
		)
	})

	Column(
		verticalArrangement = Arrangement.spacedBy(24.dp.scaledHeight(), Alignment.Top),
		horizontalAlignment = Alignment.CenterHorizontally,
		modifier = Modifier.fillMaxSize().padding(bottom = padding.calculateBottomPadding()),
	) {
		Column(
			verticalArrangement = Arrangement.spacedBy(8.dp.scaledHeight()),
			horizontalAlignment = Alignment.CenterHorizontally,
			modifier = Modifier.padding(top = 68.dp.scaledHeight()),
		) {
			ConnectionStateDisplay(connectionState = uiState.connectionState)
			uiState.stateMessage.let {
				when (it) {
					is StateMessage.Status ->
						StatusInfoLabel(
							message = it.message.asString(context),
							textColor = MaterialTheme.colorScheme.onSurfaceVariant,
						)

					is StateMessage.Error ->
						StatusInfoLabel(
							message = it.reason.toUserMessage(context),
							textColor = CustomColors.error,
						)
				}
			}
			AnimatedVisibility(visible = uiState.connectionState is ConnectionState.Connected) {
				StatusInfoLabel(
					message = uiState.connectionTime,
					textColor = MaterialTheme.colorScheme.onSurface,
				)
			}
		}
		val firstHopName = context.buildCountryNameString(appUiState.entryCountry)
		val lastHopName = context.buildCountryNameString(appUiState.exitCountry)
		val firstHopIcon = countryIcon(appUiState.entryCountry)
		val lastHopIcon = countryIcon(appUiState.exitCountry)
		Column(
			verticalArrangement = Arrangement.spacedBy(36.dp.scaledHeight(), Alignment.Bottom),
			horizontalAlignment = Alignment.CenterHorizontally,
			modifier =
			Modifier
				.fillMaxSize()
				.padding(bottom = 24.dp.scaledHeight()),
		) {
			Column(
				modifier = Modifier.padding(horizontal = 24.dp.scaledWidth()),
			) {
				Row(
					horizontalArrangement = Arrangement.SpaceBetween,
					verticalAlignment = Alignment.CenterVertically,
					modifier = Modifier
						.fillMaxWidth()
						.padding(bottom = 16.dp.scaledHeight()),
				) {
					GroupLabel(title = stringResource(R.string.select_mode))
					IconButton(onClick = {
						showDialog = true
					}, modifier = Modifier.size(iconSize)) {
						val icon = Icons.Outlined.Info
						Icon(icon, icon.name, tint = MaterialTheme.colorScheme.outline)
					}
				}
				Column(verticalArrangement = Arrangement.spacedBy(24.dp.scaledHeight(), Alignment.Bottom)) {
					IconSurfaceButton(
						leadingIcon = Icons.Outlined.VisibilityOff,
						title = stringResource(R.string.five_hop_mixnet),
						description = stringResource(R.string.five_hop_description),
						onClick = {
							if (uiState.connectionState == ConnectionState.Disconnected) {
								viewModel.onFiveHopSelected()
							} else {
								snackbar.showMessage(context.getString(R.string.disabled_while_connected))
							}
						},
						selected = appUiState.settings.vpnMode == Tunnel.Mode.FIVE_HOP_MIXNET,
					)
					IconSurfaceButton(
						leadingIcon = Icons.Outlined.Speed,
						title = stringResource(R.string.two_hop_mixnet),
						description = stringResource(R.string.two_hop_description),
						onClick = {
							if (uiState.connectionState == ConnectionState.Disconnected) {
								viewModel.onTwoHopSelected()
							} else {
								snackbar.showMessage(context.getString(R.string.disabled_while_connected))
							}
						},
						selected = appUiState.settings.vpnMode == Tunnel.Mode.TWO_HOP_MIXNET,
					)
				}
			}
			Column(
				verticalArrangement = Arrangement.spacedBy(24.dp.scaledHeight(), Alignment.Bottom),
				modifier = Modifier.padding(horizontal = 24.dp.scaledWidth()),
			) {
				GroupLabel(title = stringResource(R.string.connect_to))
				val trailingIcon = ImageVector.vectorResource(R.drawable.link_arrow_right)
				val selectionEnabled = uiState.connectionState is ConnectionState.Disconnected
				CustomTextField(
					value = firstHopName,
					readOnly = true,
					enabled = false,
					label = {
						Text(
							stringResource(R.string.first_hop),
							style = MaterialTheme.typography.bodySmall,
						)
					},
					leading = firstHopIcon,
					trailing = {
						Icon(trailingIcon, trailingIcon.name, tint = MaterialTheme.colorScheme.onSurface)
					},
					singleLine = true,
					modifier = Modifier
						.fillMaxWidth()
						.height(60.dp.scaledHeight())
						.defaultMinSize(minHeight = 1.dp, minWidth = 1.dp)
						.clickable(
							remember { MutableInteractionSource() },
							indication = if (selectionEnabled) ripple() else null,
						) {
							if (selectionEnabled) {
								navController.goFromRoot(
									Route.EntryLocation,
								)
							} else {
								snackbar.showMessage(context.getString(R.string.disabled_while_connected))
							}
						},
				)
				CustomTextField(
					value = lastHopName,
					readOnly = true,
					enabled = false,
					label = {
						Text(
							stringResource(R.string.last_hop),
							style = MaterialTheme.typography.bodySmall,
						)
					},
					leading = lastHopIcon,
					trailing = {
						Icon(trailingIcon, trailingIcon.name, tint = MaterialTheme.colorScheme.onSurface)
					},
					singleLine = true,
					modifier = Modifier
						.fillMaxWidth()
						.height(60.dp.scaledHeight())
						.defaultMinSize(minHeight = 1.dp, minWidth = 1.dp)
						.clickable(remember { MutableInteractionSource() }, indication = if (selectionEnabled) ripple() else null) {
							if (selectionEnabled) {
								navController.goFromRoot(
									Route.ExitLocation,
								)
							} else {
								snackbar.showMessage(context.getString(R.string.disabled_while_connected))
							}
						},
				)
			}
			Box(modifier = Modifier.padding(horizontal = 24.dp.scaledWidth())) {
				when (uiState.connectionState) {
					is ConnectionState.Disconnected ->
						MainStyledButton(
							testTag = Constants.CONNECT_TEST_TAG,
							onClick = {
								scope.launch {
									if (!appUiState.isMnemonicStored
									) {
										return@launch navController.goFromRoot(Route.Credential)
									}
									onConnectPressed()
								}
							},
							content = {
								Text(
									stringResource(id = R.string.connect),
									style = CustomTypography.labelHuge,
								)
							},
						)

					is ConnectionState.Disconnecting,
					is ConnectionState.Connecting,
					-> {
						val loading = ImageVector.vectorResource(R.drawable.loading)
						MainStyledButton(onClick = {}, content = { SpinningIcon(icon = loading) })
					}

					is ConnectionState.Connected ->
						MainStyledButton(
							testTag = Constants.DISCONNECT_TEST_TAG,
							onClick = { viewModel.onDisconnect() },
							content = {
								Text(
									stringResource(id = R.string.disconnect),
									style = CustomTypography.labelHuge,
								)
							},
							color = CustomColors.disconnect,
						)
				}
			}
		}
	}
}
