package io.github.mdmrk.cineo

import android.content.Context
import android.net.ConnectivityManager
import android.net.Network
import android.net.NetworkCapabilities
import android.os.Handler
import android.os.Looper

class UsableNetwork(private val onUsable: () -> Unit) {
    private var network: Long? = null
    private var validated = false

    fun changed(network: Long?, validated: Boolean) {
        val better = network != null && (network != this.network || (validated && !this.validated))
        this.network = network
        this.validated = network != null && validated
        if (better) onUsable()
    }
}

class NetworkWatch(context: Context, onUsable: () -> Unit) : ConnectivityManager.NetworkCallback() {
    private val connectivity = context.getSystemService(ConnectivityManager::class.java)
    private val usable = UsableNetwork(onUsable)

    fun start() {
        connectivity.registerDefaultNetworkCallback(this, Handler(Looper.getMainLooper()))
    }

    fun stop() {
        connectivity.unregisterNetworkCallback(this)
        usable.changed(null, false)
    }

    override fun onCapabilitiesChanged(network: Network, capabilities: NetworkCapabilities) {
        usable.changed(network.networkHandle, capabilities.hasCapability(NetworkCapabilities.NET_CAPABILITY_VALIDATED))
    }

    override fun onLost(network: Network) {
        usable.changed(null, false)
    }
}
