package app.polotno

data class VideoRequest(val asset: Asset, val muted: Boolean)

/** One decoder per active file; surface playlists keep their own selection clocks. */
fun videoPlan(layers: List<Layer>): Map<String, VideoRequest> = layers
    .filter { !it.calibration && it.asset?.kind == "video" }
    .groupBy { it.asset!!.id }
    .mapValues { (_, group) -> VideoRequest(group.first().asset!!, group.all { it.muted }) }
