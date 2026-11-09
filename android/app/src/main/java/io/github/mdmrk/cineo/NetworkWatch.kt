package io.github.mdmrk.cineo

import android.content.Context
import android.net.ConnectivityManager
import android.net.Network
import android.net.NetworkCapabilities
import android.os.Handler
import android.os.Looper
import android.os.SystemClock

private const val RENEW_AFTER_MS = 90_000L

class UsableNetwork(private val clock: () -> Long, private val onUsable: (renew: Boolean) -> Unit) {
    private var network: Long? = null
    private var validated = false
    private var stoppedAt: Long? = null

    fun stopped() {
        stoppedAt = clock()
    }

    fun changed(network: Long?, validated: Boolean) {
        val away = stoppedAt
        val moved = network != this.network
        val better = network != null && (moved || away != null || (validated && !this.validated))
        val renew = moved || (away != null && clock() - away >= RENEW_AFTER_MS)
        this.network = network
        this.validated = network != null && validated
        if (network != null) stoppedAt = null
        if (better) onUsable(renew)
    }
}

class NetworkWatch(context: Context, onUsable: (renew: Boolean) -> Unit) : ConnectivityManager.NetworkCallback() {
    private val connectivity = context.getSystemService(ConnectivityManager::class.java)
    private val usable = UsableNetwork(SystemClock::elapsedRealtime, onUsable)

    fun start() {
        connectivity.registerDefaultNetworkCallback(this, Handler(Looper.getMainLooper()))
    }

    fun stop() {
        connectivity.unregisterNetworkCallback(this)
        usable.stopped()
    }

    override fun onCapabilitiesChanged(network: Network, capabilities: NetworkCapabilities) {
        usable.changed(network.networkHandle, capabilities.hasCapability(NetworkCapabilities.NET_CAPABILITY_VALIDATED))
    }

    override fun onLost(network: Network) {
        usable.changed(null, false)
    }
}
