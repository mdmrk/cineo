package io.github.mdmrk.cineo

import android.app.Application
import io.github.mdmrk.cineo.rust.AppView
import io.github.mdmrk.cineo.rust.Cineo
import io.github.mdmrk.cineo.rust.StartException
import io.github.mdmrk.cineo.rust.StateListener
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow

class CineoApplication : Application() {
    private val views = MutableStateFlow<AppView?>(null)
    val view: StateFlow<AppView?> = views

    lateinit var started: Started
        private set

    val cineo: Cineo? get() = (started as? Started.Running)?.cineo

    override fun onCreate() {
        super.onCreate()
        Native.init(this)
        started = try {
            Started.Running(Cineo(filesDir.resolve("data").path, cacheDir.path, Views(views)))
        } catch (e: StartException) {
            Started.Failed(e.message.orEmpty())
        }
    }

    private class Views(private val flow: MutableStateFlow<AppView?>) : StateListener {
        override fun changed(view: AppView) {
            flow.value = view
        }
    }
}

sealed interface Started {
    data class Running(val cineo: Cineo) : Started
    data class Failed(val reason: String) : Started
}
