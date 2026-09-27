# Hellbyte level format

Levels are plain text files in this directory (`*.lvl`). At build time the
level compiler (`crates/mapc`) turns them into BSP trees, segs, subsectors and
a blockmap, and the result is compiled straight into the game binary. There
are no data files at run time. Levels are played in file-name order unless a
`next` line says otherwise.

## The idea: layered polygons

You describe the map by *painting regions*. Every `rect`, `poly`, `ngon` or
`stairs` paints an area with a **style** (floor/ceiling heights, textures,
light, specials). Shapes painted later cover earlier ones. `void` shapes cut
solid holes (pillars, wall thickness). The compiler works out every wall
between different regions on its own:

* region next to nothing / void -> a solid wall (`wall` texture)
* two regions with different heights -> a step, ledge, window or doorway,
  textured with the neighbour's `upper` / `lower` texture (falling back to
  the region's own `wall`)
* two regions with the same style properties -> an invisible seam

Coordinates are map units (x east, y north). The player is 32 units wide and
56 tall. Steps up to 24 units can be climbed; the eye is 41 units up.

## Commands

```
map ID "Name"                 # required, e.g. map E1M2 "Coolant Works"
sky N                         # 0 night sky, 1 red hell sky
par SECONDS                   # par time shown on the tally screen
next ID / secretnext ID       # optional level order overrides

style NAME key=value ...      # define a style (from=OTHER copies another style)
rect STYLE|void x1 y1 x2 y2 [key=value ...]
poly STYLE|void x1 y1 x2 y2 x3 y3 ... [key=value ...]
ngon STYLE|void cx cy radius sides [rot=degrees] [key=value ...]
stairs STYLE x1 y1 x2 y2 COUNT n|s|e|w [step=8]   # COUNT steps climbing towards n/s/e/w

line x1 y1 x2 y2 key=value ...     # change the walls lying along this segment
switch x1 y1 x2 y2 special=S tag=N [tex=SWITCH0]   # put a switch on a wall
trigger x1 y1 x2 y2 special=S tag=N                # walk-over line inside a room

thing KIND x y [angle] [easy] [normal] [hard] [ambush] [multi]
```

Region keys (in `style` or overriding a single shape): `floor`, `ceil`,
`light` (0-255), `ftex`, `ctex` (a flat, or `SKY`), `wall`, `upper`, `lower`,
`special` (sector special), `tag`, `door` (line special put on every line
around the region, together with the region's tag), `as=NAME` (shapes with the
same name share one sector).

Line keys: `special`, `tag`, `upper`, `mid`, `lower` (or `front.upper`,
`back.mid`, ...), `xoff`, `yoff`, `flags=impassable,blockmonsters,
upperunpegged,lowerunpegged,secret,blocksound,hidden,mapped`.

## Useful patterns

* **Door:** a thin region between two rooms with `floor == ceil`, `wall=DOORTRAK`,
  `upper=DOOR1`, `door=door_repeat`. Locked: `upper=DOORRED door=door_red_repeat`.
  Doors that open onto a tall space look best inside a low "frame" region.
* **Lift:** `door=sr_lift tag=N` on the lift region: using any of its edges
  lowers it to the lowest neighbouring floor; it rises again after 3 seconds.
* **Window:** a thin region with a raised floor and lowered ceiling between two rooms.
* **Secret:** `special=secret` on a room; hide the entrance with a door whose
  `upper` matches the wall, and mark the line `flags=secret` for the automap.
* **Exit:** `switch x1 y1 x2 y2 special=s1_exit tex=EXITSW0`.
* **Teleporter:** `trigger` lines with `special=wr_teleport tag=N` and a
  `thing teleport_dest` inside the region tagged N.
* **Boss door:** regions tagged 666 lower their floor when the last juggernaut dies.

## Names

Line specials: `door_repeat door_repeat_fast door_stay door_{red,blue,yellow}_{repeat,stay}
w1_door_open wr_door_open s1_door_open sr_door_open w1_door_open_stay s1_door_open_stay
sr_door_open_stay g1_door_open_stay w1_door_close s1_door_close w1_lift wr_lift s1_lift
sr_lift w1_floor_lower_lowest s1_floor_lower_lowest w1_floor_raise_nearest
s1_floor_raise_nearest w1_floor_raise24 s1_floor_raise24 w1_floor_raise_highest
w1_floor_raise_ceiling s1_floor_raise_ceiling w1_ceiling_lower_floor w1_crusher wr_crusher
w1_crusher_stop w1_stairs8 s1_stairs8 w1_light_off w1_light_on w1_teleport wr_teleport
s1_exit w1_exit s1_secret_exit w1_secret_exit scroll_left`

Sector specials: `light_flicker light_strobe_fast light_strobe_slow light_glow damage5
damage10 damage20 secret door_close30 exit_damage`

Things: `player1 teleport_dest` · monsters `drone enforcer heavy fiend ripper gazer
juggernaut` · weapons `shotgun chaingun launcher plasma arc_cannon drill` · ammo `clip
ammo_box shells shell_box rockets rocket_box cell cell_pack backpack` · health/armor
`health_bonus stimpack medkit vital_orb armor_bonus armor_green armor_blue` · keys
`key_red key_blue key_yellow` · powers `aegis hazmat survey_map adrenaline` · scenery
`barrel lamp pillar torch corpse`

Wall textures: `BASE1 BASE2 BASE3 STONE1 STONE2 BRICK1 BRICK2 TECH1 COMP1 WOOD1 MARBLE1
ROCK1 ROCK2 FLESH1 LAVAWALL DOOR1 DOORRED DOORBLU DOORYEL DOORTRAK SWITCH0 SWITCH1 EXITSW0
EXITSW1 SUPPORT1 STEP1 LIFT1 PIPES1 CRATE1 LIGHT1 EXITSIGN CEMENT1 METAL1 HELLROCK GRATE1`
(`GRATE1` has see-through holes: use it as a `mid` texture on two-sided lines.)

Flats: `FLOOR1 FLOOR2 FLOOR3 FLOOR4 CEIL1 CEIL2 CEILLITE NUKAGE1 LAVA1 WATER1 DIRT1
ROCKFLR TELEPAD STEPTOP CRATETOP WOODFLR HELLFLR MARBFLR GRASS1 CEMENTF SKY`
(liquids animate automatically).

All art is generated from code in `crates/engine/build/textures.rs`; set
`HELLBYTE_DUMP=/some/dir` while building to get PNG sheets of every texture.
