package net.nymtech.vpn

import android.content.Context
import android.content.Intent
import android.net.VpnService
import android.os.Build
import android.os.IBinder
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import net.nymtech.vpn.model.BackendMessage
import net.nymtech.vpn.model.Statistics
import net.nymtech.vpn.util.Constants
import net.nymtech.vpn.util.InvalidCredentialException
import net.nymtech.vpn.util.SingletonHolder
import net.nymtech.vpn.util.prefix
import nym_vpn_lib.AndroidTunProvider
import nym_vpn_lib.BandwidthStatus
import nym_vpn_lib.ConnectionStatus
import nym_vpn_lib.ExitStatus
import nym_vpn_lib.FfiException
import nym_vpn_lib.NymConfig
import nym_vpn_lib.NymVpnStatus
import nym_vpn_lib.TunStatus
import nym_vpn_lib.TunnelStatusListener
import nym_vpn_lib.VpnConfig
import nym_vpn_lib.WgConfig
import nym_vpn_lib.checkCredential
import nym_vpn_lib.initLogger
import nym_vpn_lib.runVpn
import nym_vpn_lib.stopVpn
import timber.log.Timber
import java.net.InetAddress
import java.security.Provider.Service
import java.time.Instant

class NymBackend private constructor(val context: Context) : Backend, TunnelStatusListener {

	init {
		System.loadLibrary(Constants.NYM_VPN_LIB)
		initLogger("info")
	}

	companion object : SingletonHolder<NymBackend, Context>(::NymBackend) {
		private var vpnService = CompletableDeferred<VpnService>()
		private var currentTunnelHandle: Int = -1
	}

	private val ioDispatcher = Dispatchers.IO

	private var statsJob: Job? = null
	private var tunnel: Tunnel? = null
	private var state: Tunnel.State = Tunnel.State.Down

	override suspend fun validateCredential(credential: String): Instant? {
		return try {
			withContext(ioDispatcher) {
				checkCredential(credential)
			}
		} catch (e: FfiException) {
			Timber.e(e)
			throw InvalidCredentialException("Credential invalid or expired")
		}
	}

	override suspend fun importCredential(credential: String): Instant? {
		return try {
			nym_vpn_lib.importCredential(credential, Constants.NATIVE_STORAGE_PATH)
		} catch (e: FfiException) {
			Timber.e(e)
			throw InvalidCredentialException("Credential invalid or expired")
		}
	}

	override suspend fun start(tunnel: Tunnel, background: Boolean): Tunnel.State {
		Timber.d("Starting tunnel now")
		val state = getState()
		if (tunnel == this.tunnel && state != Tunnel.State.Down) return state
		this.tunnel = tunnel
		Timber.d("Starting tunnel now1")
		tunnel.environment.setup()
		if (!vpnService.isCompleted) {
			Timber.d("Trying to start the service")
			kotlin.runCatching {
				if (background) {
					if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
						context.startForegroundService(Intent(context, VpnBackgroundService::class.java))
					} else {
						context.startService(Intent(context, VpnBackgroundService::class.java))
					}
				}
				context.startService(Intent(context, VpnService::class.java))
			}.onFailure { Timber.w("Ignoring not started in time exception") }
		}
		Timber.d("Starting tunnel now2")
		// reset any error state
		tunnel.onBackendMessage(BackendMessage.None)
		withContext(ioDispatcher) {
			Timber.d("Starting tunnel now3")
			val service = vpnService.await()
			service.setOwner(this@NymBackend)
			runCatching {
				runVpn(
					VpnConfig(
						tunnel.environment.apiUrl,
						tunnel.environment.nymVpnApiUrl,
						tunnel.entryPoint,
						tunnel.exitPoint,
						isTwoHop(tunnel.mode),
						service,
						Constants.NATIVE_STORAGE_PATH,
						this@NymBackend,
					),
				)
			}.onFailure {
				Timber.e(it)
				// temp for now until we setup error/message callback
				tunnel.onBackendMessage(BackendMessage.Error.StartFailed)
			}
		}
		return Tunnel.State.Connecting.InitializingClient
	}

	@OptIn(ExperimentalCoroutinesApi::class)
	override suspend fun stop(): Tunnel.State {
		withContext(ioDispatcher) {
			stopVpn()
			currentTunnelHandle = -1
			vpnService.getCompleted().stopSelf()
		}
		return Tunnel.State.Disconnecting
	}

	private fun onDisconnect() {
		statsJob?.cancel()
		tunnel?.onStatisticChange(Statistics())
	}

	private fun onConnect() = CoroutineScope(ioDispatcher).launch {
		startConnectionTimer()
	}

	override fun getState(): Tunnel.State {
		return state
	}

	private fun isTwoHop(mode: Tunnel.Mode): Boolean = when (mode) {
		Tunnel.Mode.TWO_HOP_MIXNET -> true
		else -> false
	}

	private suspend fun startConnectionTimer() {
		withContext(ioDispatcher) {
			var seconds = 0L
			do {
				if (state == Tunnel.State.Up) {
					tunnel?.onStatisticChange(Statistics(seconds))
					seconds++
				}
				delay(Constants.STATISTICS_INTERVAL_MILLI)
			} while (true)
		}
	}

	override fun onTunStatusChange(status: TunStatus) {
		val state = when (status) {
			TunStatus.INITIALIZING_CLIENT -> Tunnel.State.Connecting.InitializingClient
			TunStatus.ESTABLISHING_CONNECTION -> Tunnel.State.Connecting.EstablishingConnection
			TunStatus.DOWN -> {
				Tunnel.State.Down
			}
			TunStatus.UP -> {
				statsJob = onConnect()
				Tunnel.State.Up
			}
			TunStatus.DISCONNECTING -> {
				onDisconnect()
				Tunnel.State.Disconnecting
			}
		}
		this.state = state
		tunnel?.onStateChange(state)
	}

	override fun onBandwidthStatusChange(status: BandwidthStatus) {
		Timber.d("Bandwidth status: $status")
	}

	override fun onConnectionStatusChange(status: ConnectionStatus) {
		Timber.d("Connection status: $status")
	}

	override fun onNymVpnStatusChange(status: NymVpnStatus) {
		Timber.d("VPN status: $status")
	}

	override fun onExitStatusChange(status: ExitStatus) {
		when (status) {
			ExitStatus.Stopped -> Timber.d("Tunnel stopped")
			is ExitStatus.Failed -> {
				Timber.e(status.error)
				// need to stop the vpn service even though vpn never started from lib perspective
				context.stopService(Intent(context, VpnService::class.java))
				tunnel?.onBackendMessage(BackendMessage.Error.StartFailed)
				// Need to set state down because this likely never happened in lib
				tunnel?.onStateChange(Tunnel.State.Down)
			}
		}
	}

	class VpnService : android.net.VpnService(), AndroidTunProvider {
		private var owner: NymBackend? = null

		val builder: Builder
			get() = Builder()

		override fun onCreate() {
			Timber.d("Vpn service created")
			vpnService.complete(this)
			super.onCreate()
		}

		override fun onDestroy() {
			currentTunnelHandle = -1
			vpnService = CompletableDeferred()
			super.onDestroy()
		}

		override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
			Timber.d("Vpn service on start")
			vpnService.complete(this)
			// TODO can add AOVPN callback here later
			return super.onStartCommand(intent, flags, startId)
		}

		fun setOwner(owner: NymBackend?) {
			this.owner = owner
		}

		override fun bypass(socket: Int) {
			protect(socket)
		}

		override fun configureWg(config: WgConfig) {
		}

		override fun configureNym(config: NymConfig): Int {
			Timber.d("Configuring Nym tunnel")
			if (prepare(this) != null) return -1
			if (currentTunnelHandle != -1) return currentTunnelHandle
			val vpnInterface = builder.apply {
				addAddress(config.ipv4Addr, InetAddress.getByName(config.ipv4Addr).prefix())
				addAddress(config.ipv6Addr, InetAddress.getByName(config.ipv6Addr).prefix())
				config.dnsIps.forEach {
					addDnsServer(it)
				}
				try {
					val allowedIps = config.allowedIps.map { it.split("/") }
					allowedIps.forEach {
						addRoute(it.first(), it.last().toInt())
						addRoute(it.first(), it.last().toInt())
					}
				} catch (e: Exception) {
					Timber.e(e)
					return -1
				}

				setMtu(config.mtu.toInt())

				setBlocking(false)
				if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
					setMetered(false)
				}
			}.establish()
			val fd = vpnInterface?.detachFd() ?: return -1
			return fd
		}
	}

	class VpnBackgroundService : android.app.Service() {
		override fun onBind(intent: Intent?): IBinder? {
			return null
		}

		override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
			startService(Intent(this, VpnService::class.java))
			startForeground(123, NotificationManager.createVpnRunningNotification(this))
			return START_NOT_STICKY
		}

		override fun onDestroy() {
			super.onDestroy()
			Timber.d("Wrapper got destroyed, but who cares")
		}
	}
}
