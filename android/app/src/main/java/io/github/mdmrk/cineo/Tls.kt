package io.github.mdmrk.cineo

import android.content.Context

object Tls {
    init {
        System.loadLibrary("cineo_android")
    }

    @JvmStatic
    external fun init(context: Context)
}
