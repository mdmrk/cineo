package io.github.mdmrk.cineo.ui

internal fun playerStarted(was: Boolean, loaded: Boolean, waiting: Boolean): Boolean =
    loaded && (was || !waiting)
