package io.github.mdmrk.cineo.ui

import io.github.mdmrk.cineo.R

fun languageName(code: String): Int? = when (code) {
    "ara" -> R.string.language_ara
    "bul" -> R.string.language_bul
    "cat" -> R.string.language_cat
    "chi" -> R.string.language_chi
    "hrv" -> R.string.language_hrv
    "cze" -> R.string.language_cze
    "dan" -> R.string.language_dan
    "dut" -> R.string.language_dut
    "eng" -> R.string.language_eng
    "fin" -> R.string.language_fin
    "fre" -> R.string.language_fre
    "ger" -> R.string.language_ger
    "gre" -> R.string.language_gre
    "heb" -> R.string.language_heb
    "hin" -> R.string.language_hin
    "hun" -> R.string.language_hun
    "ind" -> R.string.language_ind
    "ita" -> R.string.language_ita
    "jpn" -> R.string.language_jpn
    "kor" -> R.string.language_kor
    "nor" -> R.string.language_nor
    "per" -> R.string.language_per
    "pol" -> R.string.language_pol
    "por" -> R.string.language_por
    "rum" -> R.string.language_rum
    "rus" -> R.string.language_rus
    "srp" -> R.string.language_srp
    "spa" -> R.string.language_spa
    "swe" -> R.string.language_swe
    "tha" -> R.string.language_tha
    "tur" -> R.string.language_tur
    "ukr" -> R.string.language_ukr
    "vie" -> R.string.language_vie
    else -> null
}
