package app.polotno

import org.json.JSONObject

data class Point(val x: Float, val y: Float)
data class Asset(val id: String, val kind: String)
data class NeonFrame(val color: String, val width: Float, val glow: Float)
data class Region(val id: String, val corners: List<Point>, val items: List<String>, val intervalMs: Long, val muted: Boolean, val frame: NeonFrame? = null, val fit: String = "contain", val calibration:Boolean = false, val flipHorizontal:Boolean=false, val flipVertical:Boolean=false)
data class Snapshot(val id: String, val revision: Long, val paused: Boolean, val regions: List<Region>, val assets: Map<String, Asset>, val controllerEpoch: Long = 0) {
    companion object {
        fun parse(json: String): Snapshot {
            val root = JSONObject(json)
            val scene = root.getJSONObject("scene")
            val assets = root.getJSONArray("assets")
            val regions = scene.getJSONArray("surfaces")
            return Snapshot(scene.getString("id"), root.getLong("revision"), root.getBoolean("paused"),
                List(regions.length()) { i ->
                    val r = regions.getJSONObject(i)
                    val corners = r.getJSONArray("corners")
                    val p = r.getJSONObject("playlist")
                    val items = p.getJSONArray("items")
                    Region(r.getString("id"), List(4) { j -> corners.getJSONObject(j).let { Point(it.getDouble("x").toFloat(), it.getDouble("y").toFloat()) } },
                        List(items.length()) { j -> items.getString(j) }, p.getLong("interval_seconds") * 1000, p.getBoolean("muted"), r.optJSONObject("frame")?.let { NeonFrame(it.getString("color"),it.getDouble("width").toFloat(),it.getDouble("glow").toFloat()) }, r.optString("fit","contain"),r.optBoolean("calibration",false),r.optBoolean("flip_horizontal",false),r.optBoolean("flip_vertical",false))
                }, List(assets.length()) { i -> assets.getJSONObject(i).let { Asset(it.getString("id"), it.getString("kind")) } }.associateBy { it.id }, root.optLong("controller_epoch",0))
        }
    }
}

/** Independent region clocks use monotonic active time, so pausing freezes every playlist. */
class PlaylistClock {
    private data class Clock(val items: List<String>, val interval: Long, val started: Long)
    private val clocks = mutableMapOf<String, Clock>()
    private var previousTime: Long? = null
    private var wasPaused = false
    private var activeTime = 0L
    private var sceneId: String? = null
    fun items(snapshot: Snapshot, nowMs: Long): Map<String, String?> {
        previousTime?.let { if (!wasPaused) activeTime += (nowMs - it).coerceAtLeast(0) }
        previousTime = nowMs
        wasPaused = snapshot.paused
        if (sceneId != snapshot.id) { clocks.clear(); sceneId = snapshot.id }
        clocks.keys.retainAll(snapshot.regions.map { it.id }.toSet())
        return snapshot.regions.associate { r ->
            val old = clocks[r.id]
            val clock = if (old == null || old.items != r.items || old.interval != r.intervalMs) Clock(r.items, r.intervalMs, activeTime).also { clocks[r.id] = it } else old
            r.id to if (r.items.isEmpty()) null else r.items[((activeTime - clock.started) / r.intervalMs % r.items.size).toInt()]
        }
    }
}
