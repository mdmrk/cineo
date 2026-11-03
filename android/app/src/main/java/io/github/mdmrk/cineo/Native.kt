package io.github.mdmrk.cineo

import android.content.Context
import android.view.Surface

object Native {
    init {
        System.loadLibrary("cineo_android")
    }

    @JvmStatic
    external fun init(context: Context)

    @JvmStatic
    external fun attachSurface(surface: Surface): Long

    @JvmStatic
    external fun releaseSurface()
}
