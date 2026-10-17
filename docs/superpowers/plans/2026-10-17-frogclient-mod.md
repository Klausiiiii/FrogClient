# FrogClient Ingame-Mod – Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Eine client-only Fabric-Mod für Minecraft 26.3 mit Client-GUI (zwei umschaltbare Layouts), Zoom, FPS-Anzeige, Fullbright und HUD-Editor, gebaut auf einem deklarativen Modul-/Settings-Gerüst.

**Architecture:** Reine Logik (Settings, Config-Persistenz, Zoom-Mathe, HUD-Positionen) liegt in Klassen ohne Minecraft-Abhängigkeit und ist per JUnit getestet. Minecraft-Anbindung erfolgt über Fabric-API-Events und wenige Mixins. Beide GUI-Layouts rendern ausschließlich aus den Modul-Deklarationen über dieselben Setting-Widgets.

**Tech Stack:** Java 25, Gradle 9.7.1 (Wrapper), Fabric Loom (`net.fabricmc.fabric-loom` 1.18-SNAPSHOT), Fabric Loader 0.19.5, Fabric API 0.163.0+26.3, Mojang-Namen (26.x ist unobfuskiert, keine Mappings), Gson (von Minecraft mitgeliefert), MixinExtras (im Loader enthalten), JUnit 5.

**Spec:** `docs/superpowers/specs/2026-10-17-frogclient-design.md` (Abschnitt 6.3 + Verzeichnisaufbau in 6.2)

## Global Constraints

- Minecraft `26.3`, Java `25`, `fabricloader >=0.19.5`, `fabric-api` Pflicht.
- Mod-ID `frogclient`, Paket `dev.frogclient`, `"environment": "client"` (nur Client-Entrypoint).
- Projekt liegt in `/mod` im Monorepo. Alle Gradle-Befehle aus `G:\Projects\FrogClient\mod` mit `JAVA_HOME="C:\Program Files\Java\jdk-25.0.4"` (System-Default ist Java 21).
- Config-Datei: `<configDir>/frogclient.json`; `configDir` = System-Property `frogclient.configDir`, sonst `FabricLoader.getInstance().getConfigDir()`. Schreiben immer atomar (temp + move).
- Akzentfarbe `#5fd068` (`0xFF5FD068`), GUI dunkel/halbtransparent.
- Keybind-Kategorie „FrogClient“: GUI öffnen = **Rechte Umschalt** (`GLFW_KEY_RIGHT_SHIFT`), Zoom = **C** (`GLFW_KEY_C`). Beide in den Minecraft-Steuerungseinstellungen änderbar.
- Kein Code darf `options.txt`-Werte dauerhaft verändern (Gamma, Smooth Camera, FOV).
- Inaktive Module registrieren keine Arbeit pro Tick/Frame; keine Allokationen pro Frame im HUD.
- Sprachdateien `en_us.json` und `de_de.json`; alle UI-Texte über Übersetzungsschlüssel `frogclient.*` / `key.frogclient.*`.
- Mojang-Klassen-/Methodennamen in diesem Plan stammen aus 1.21.x; vor jedem Mixin mit `gradlew genSources` die echten 26.3-Namen prüfen und ggf. anpassen.

## Review Focus

1. **Kaputte/teilweise/alte `frogclient.json`** (z. B. zwei Instanzen schreiben gleichzeitig, Nutzer editiert von Hand) → Client startet trotzdem; ungültige Teile fallen auf Defaults, die kaputte Datei wird als `frogclient.json.bak` gesichert. (Task 2)
2. **Fenstergröße/GUI-Scale ändert sich** → HUD-Elemente und GUI-Panels bleiben vollständig sichtbar. (Task 5, Task 6)
3. **Zoom-Taste gehalten, während ein Screen/Chat öffnet oder das Fenster den Fokus verliert** → Zoom endet, Sensitivität und Smooth-Camera sind wiederhergestellt; Tippen von „c“ im Chat zoomt nie. (Task 4)
4. **Fullbright an und Spiel wird beendet** → `options.txt` enthält danach denselben Gamma-Wert wie vorher. (Task 3)
5. **Slider-Wert außerhalb des Bereichs in der Config** (z. B. `zoom.factor: 999`) → wird beim Laden auf Bereich geklemmt und auf Schrittweite gerundet. (Task 1)

---

## Dateistruktur

```
mod/
  settings.gradle, build.gradle, gradle.properties, gradlew, gradlew.bat, gradle/wrapper/*
  src/main/resources/
    fabric.mod.json, frogclient.mixins.json
    assets/frogclient/icon.png, assets/frogclient/lang/en_us.json, de_de.json
  src/main/java/dev/frogclient/
    FrogClient.java                 ClientModInitializer: verdrahtet alles
    FrogKeys.java                   Registrierung der KeyMappings
    setting/Setting.java            sealed interface
    setting/BoolSetting.java, SliderSetting.java, EnumSetting.java, ColorSetting.java
    module/Category.java            enum RENDER, HUD, SETTINGS
    module/Module.java              abstrakte Basis
    module/ModuleManager.java       Registry
    config/ConfigStore.java         Laden/Speichern JSON
    config/GuiState.java            Layout-Wahl + Panel-Positionen
    feature/FullbrightModule.java
    feature/zoom/ZoomMath.java      reine Mathe
    feature/zoom/ZoomModule.java    Zustand + Tick-Logik
    feature/fps/FpsModule.java
    hud/HudPosition.java            reine Mathe (relativ ↔ Pixel)
    hud/HudElement.java             Interface für verschiebbare HUD-Elemente
    hud/HudRenderer.java            Fabric-HUD-Anbindung
    hud/HudEditorScreen.java
    gui/ClientScreen.java           Screen mit Layout-Umschalter
    gui/GuiLayout.java              Interface
    gui/PanelLayout.java            Layout A
    gui/WindowLayout.java           Layout B
    gui/widget/SettingWidget.java   + ToggleWidget, SliderWidget, EnumWidget, ColorWidget, WidgetFactory
    mixin/GameRendererMixin.java    FOV
    mixin/MouseHandlerMixin.java    Sensitivität + Scroll
    mixin/LightmapMixin.java        Gamma
  src/test/java/dev/frogclient/...  JUnit-Tests der reinen Logik
```

---

### Task 1: Projektgerüst + Settings-Modell

**Files:**
- Create: `mod/settings.gradle`, `mod/build.gradle`, `mod/gradle.properties`, Wrapper-Dateien, `mod/src/main/resources/fabric.mod.json`, `mod/src/main/resources/frogclient.mixins.json`, `mod/src/main/resources/assets/frogclient/lang/en_us.json`, `de_de.json`, `icon.png`
- Create: `mod/src/main/java/dev/frogclient/FrogClient.java`
- Create: `mod/src/main/java/dev/frogclient/setting/{Setting,BoolSetting,SliderSetting,EnumSetting,ColorSetting}.java`
- Test: `mod/src/test/java/dev/frogclient/setting/SettingTest.java`

**Interfaces:**
- Produces:
  - `sealed interface Setting<T> permits BoolSetting, SliderSetting, EnumSetting, ColorSetting` mit `String id()`, `String nameKey()`, `T get()`, `void set(T value)`, `T defaultValue()`, `void reset()`, `JsonElement toJson()`, `void fromJson(JsonElement json)` (ungültig/null → Default, wirft nie).
  - `BoolSetting(String id, String nameKey, boolean def)`
  - `SliderSetting(String id, String nameKey, double min, double max, double step, double def)`; `set` klemmt auf `[min,max]` und rundet auf `min + k*step`; `double min()`, `max()`, `step()`.
  - `EnumSetting<E extends Enum<E>>(String id, String nameKey, Class<E> type, E def)`; JSON = `name()`; `E next()` zyklisch.
  - `ColorSetting(String id, String nameKey, int defArgb)`; JSON = `"#AARRGGBB"`; `static final int[] PRESETS` = `{0xFFFFFFFF, 0xFF5FD068, 0xFFFF5555, 0xFFFFAA00, 0xFFFFFF55, 0xFF55FFFF, 0xFF5555FF, 0xFFFF55FF}`; `int next()` springt zum nächsten Preset.
  - `FrogClient implements ClientModInitializer` mit `static final Logger LOG` (Name `"FrogClient"`).

- [ ] **Step 1: Gradle-Projekt anlegen.** Wrapper (`gradlew`, `gradlew.bat`, `gradle/wrapper/gradle-wrapper.jar`, `gradle-wrapper.properties` mit Gradle 9.7.1) aus `FabricMC/fabric-example-mod` (Branch HEAD) übernehmen. `build.gradle` wie dort, aber **ohne** `splitEnvironmentSourceSets`/`mods {}`-Block und ohne `maven-publish`; zusätzlich:

```groovy
dependencies {
    testImplementation platform("org.junit:junit-bom:5.13.4")
    testImplementation "org.junit.jupiter:junit-jupiter"
    testRuntimeOnly "org.junit.platform:junit-platform-launcher"
}
test { useJUnitPlatform() }
```

`gradle.properties`: `minecraft_version=26.3`, `loader_version=0.19.5`, `loom_version=1.18-SNAPSHOT`, `fabric_api_version=0.163.0+26.3`, `version=0.1.0`, `group=dev.frogclient`, `archives_base_name=frogclient`. `settings.gradle`: `rootProject.name = 'frogclient'`.

- [ ] **Step 2: `fabric.mod.json`** mit `id: frogclient`, `name: FrogClient`, `environment: client`, einzigem Entrypoint `client: ["dev.frogclient.FrogClient"]`, `mixins: ["frogclient.mixins.json"]`, `depends` wie Template (`minecraft ~26.3`, `java >=25`, `fabricloader >=0.19.5`, `fabric-api *`), `contact.sources` = `https://github.com/Klausiiiii/FrogClient`. Mixin-Config mit Paket `dev.frogclient.mixin`, `client: []` (wird später gefüllt), `compatibilityLevel: JAVA_25`. Icon: 128×128 PNG, grüner Frosch-Platzhalter. `FrogClient.onInitializeClient()` loggt `"FrogClient {} geladen"` mit Version.

- [ ] **Step 3: Failing Tests schreiben** (`SettingTest`):

```java
@Test void sliderClampsAndSnaps() {
    var s = new SliderSetting("factor", "k", 2, 10, 0.5, 4);
    s.set(999.0);  assertEquals(10.0, s.get());
    s.set(-3.0);   assertEquals(2.0, s.get());
    s.set(4.3);    assertEquals(4.5, s.get());
}
@Test void sliderFromJsonClampsOutOfRange() {
    var s = new SliderSetting("factor", "k", 2, 10, 0.5, 4);
    s.fromJson(new JsonPrimitive(999)); assertEquals(10.0, s.get());
}
@Test void invalidJsonFallsBackToDefault() {
    var b = new BoolSetting("x", "k", true);
    b.set(false); b.fromJson(new JsonPrimitive("kaputt")); assertTrue(b.get());
    var e = new EnumSetting<>("e", "k", TestEnum.class, TestEnum.A);
    e.fromJson(new JsonPrimitive("GIBTSNICHT")); assertEquals(TestEnum.A, e.get());
    var c = new ColorSetting("c", "k", 0xFFFFFFFF);
    c.fromJson(JsonNull.INSTANCE); assertEquals(0xFFFFFFFF, c.get());
}
@Test void roundTripAllTypes() { /* set non-default, toJson, fromJson into fresh instance, equal */ }
@Test void colorJsonFormat() { assertEquals("#FF5FD068", new ColorSetting("c","k",0xFF5FD068).toJson().getAsString()); }
@Test void enumNextCycles() { /* A→B→A */ }
@Test void colorNextCyclesPresets() { /* 0xFFFFFFFF → 0xFF5FD068; letzter → erster; Nicht-Preset → erster */ }
```

- [ ] **Step 4: Tests laufen lassen, Fehlschlag prüfen.** Run: `JAVA_HOME="C:/Program Files/Java/jdk-25.0.4" ./gradlew test` → FAIL (Klassen fehlen). Erster Lauf lädt Minecraft herunter, dauert.
- [ ] **Step 5: Setting-Klassen implementieren** wie in Interfaces beschrieben.
- [ ] **Step 6: Tests + Build grün.** Run: `./gradlew test build` → BUILD SUCCESSFUL, `mod/build/libs/frogclient-0.1.0.jar` existiert.
- [ ] **Step 7: Spielstart prüfen.** Run: `./gradlew runClient` → Titelbildschirm erscheint, Log enthält „FrogClient 0.1.0 geladen“. Spiel schließen. `mod/run/` und `mod/build/` in `/.gitignore` aufnehmen (zusätzlich `.gradle/`).
- [ ] **Step 8: Commit** `feat(mod): scaffold Fabric mod and settings model`

---

### Task 2: Module, ModuleManager, Config-Persistenz

**Files:**
- Create: `mod/src/main/java/dev/frogclient/module/{Category,Module,ModuleManager}.java`
- Create: `mod/src/main/java/dev/frogclient/config/{ConfigStore,GuiState}.java`
- Test: `mod/src/test/java/dev/frogclient/config/ConfigStoreTest.java`, `mod/src/test/java/dev/frogclient/module/ModuleTest.java`

**Interfaces:**
- Consumes: `Setting<T>` (Task 1).
- Produces:
  - `enum Category { RENDER, HUD, SETTINGS }` mit `String nameKey()` → `"frogclient.category.<lowercase>"`.
  - `abstract class Module` – Konstruktor `Module(String id, Category category, boolean defaultEnabled)`; `String id()`, `String nameKey()` (`"frogclient.module.<id>"`), `Category category()`, `boolean isEnabled()`, `void setEnabled(boolean)` (ruft bei Wechsel `onEnable()`/`onDisable()`), `void toggle()`, `List<Setting<?>> settings()`, `protected <S extends Setting<?>> S add(S setting)`, `protected void onEnable() {}`, `protected void onDisable() {}`.
  - `final class ModuleManager` – `void register(Module)`, `List<Module> all()` (Registrierungsreihenfolge), `List<Module> byCategory(Category)`, `<M extends Module> M get(Class<M>)`.
  - `final class GuiState` – `enum Layout { PANELS, WINDOW }`; Felder mit Gettern/Settern: `Layout layout` (Default `PANELS`), `Map<Category, int[]> panelPositions` (x,y in GUI-Pixeln; leer = Auto-Anordnung), `Map<String, double[]> hudPositions` (relativ 0..1).
  - `final class ConfigStore` – `ConfigStore(Path dir)`; `static Path resolveDir()` (Property `frogclient.configDir`, sonst Fabric-Config-Dir); `void load(ModuleManager, GuiState)`; `void save(ModuleManager, GuiState)`; `Path file()` = `dir/frogclient.json`.
- JSON-Schema (Version 1):

```json
{ "version": 1,
  "gui": { "layout": "PANELS", "panels": { "RENDER": [10, 10] } },
  "hud": { "fps": [0.01, 0.01] },
  "modules": { "zoom": { "enabled": true, "settings": { "factor": 4.0 } } } }
```

- [ ] **Step 1: Failing Tests schreiben:**

```java
@Test void saveThenLoadRestoresEverything(@TempDir Path dir)   // enabled, Settings, layout=WINDOW, panel- & hud-Positionen
@Test void missingFileGivesDefaults(@TempDir Path dir)          // kein Fehler, alles Default, keine Datei angelegt
@Test void corruptFileGivesDefaultsAndBackup(@TempDir Path dir) // Datei "{nicht json" → Defaults; frogclient.json.bak existiert mit Originalinhalt
@Test void partiallyInvalidKeepsValidParts(@TempDir Path dir)   // modules.zoom.settings.factor="x", layout="WINDOW" → factor Default, layout WINDOW
@Test void unknownModulesAndKeysIgnored(@TempDir Path dir)
@Test void saveIsAtomic(@TempDir Path dir)                      // nach save keine *.tmp-Datei übrig; Datei parsebar
@Test void resolveDirUsesSystemProperty()                       // Property setzen → Pfad; danach Property entfernen
// ModuleTest
@Test void setEnabledCallsHooksOnlyOnChange()                   // setEnabled(true) zweimal → onEnable einmal
@Test void byCategoryKeepsRegistrationOrder()
```

- [ ] **Step 2: Tests ausführen → FAIL.** Run: `./gradlew test`
- [ ] **Step 3: Implementieren.** Gson mit `setPrettyPrinting()`. `save`: in `frogclient.json.tmp` schreiben, dann `Files.move(tmp, file, ATOMIC_MOVE, REPLACE_EXISTING)`, bei `AtomicMoveNotSupportedException` ohne `ATOMIC_MOVE`. `load` fängt jede Exception pro Abschnitt ab; bei nicht parsebarer Datei: nach `.bak` kopieren (überschreiben), loggen, Defaults behalten. `ModuleManager` hält `LinkedHashMap`.
- [ ] **Step 4: Tests → PASS.** Run: `./gradlew test`
- [ ] **Step 5: In `FrogClient` verdrahten:** statische `ModuleManager modules`, `GuiState gui`, `ConfigStore config`; in `onInitializeClient` laden; speichern bei `ClientLifecycleEvents.CLIENT_STOPPING`. Statischer Helfer `FrogClient.saveConfig()` für spätere Tasks.
- [ ] **Step 6: Commit** `feat(mod): module registry and atomic JSON config`

---

### Task 3: Keybinds + Fullbright

**Files:**
- Create: `mod/src/main/java/dev/frogclient/FrogKeys.java`, `mod/src/main/java/dev/frogclient/feature/FullbrightModule.java`, `mod/src/main/java/dev/frogclient/mixin/LightmapMixin.java`
- Modify: `FrogClient.java`, `frogclient.mixins.json`, Sprachdateien
- Test: `mod/src/test/java/dev/frogclient/feature/FullbrightModuleTest.java`

**Interfaces:**
- Consumes: `Module`, `ModuleManager`, `FrogClient.modules`.
- Produces:
  - `final class FrogKeys` – `static KeyMapping OPEN_GUI`, `static KeyMapping ZOOM`; `static void register()`. Registrierung über den Fabric-API-Keybinding-Helper (26.3-Name per genSources prüfen: `KeyBindingHelper` bzw. `KeyMappingHelper`), Kategorie „FrogClient“ (`key.category.frogclient` bzw. 26.x-`KeyMapping.Category`-API). Defaults: `GLFW_KEY_RIGHT_SHIFT`, `GLFW_KEY_C`.
  - `final class FullbrightModule extends Module` – id `"fullbright"`, `Category.RENDER`, default aus; `static final double GAMMA_OVERRIDE = 16.0`; `double effectiveGamma(double optionGamma)` → bei an `GAMMA_OVERRIDE`, sonst `optionGamma`.

- [ ] **Step 1: Failing Test:**

```java
@Test void effectiveGammaOnlyOverridesWhenEnabled() {
    var m = new FullbrightModule();
    assertEquals(0.5, m.effectiveGamma(0.5));
    m.setEnabled(true);  assertEquals(16.0, m.effectiveGamma(0.5));
    m.setEnabled(false); assertEquals(0.5, m.effectiveGamma(0.5));
}
```

- [ ] **Step 2: Test → FAIL**, dann `FullbrightModule` implementieren, Test → PASS.
- [ ] **Step 3: `LightmapMixin`.** Per `genSources` die Stelle finden, an der die Lightmap den Gamma-Wert aus `Options.gamma()` liest (1.21.x: `LightTexture#updateLightTexture`). Mit MixinExtras `@ModifyExpressionValue` (oder `@ModifyArg` auf den Uniform-Upload, je nach 26.3-Code) den gelesenen Wert durch `FrogClient.modules.get(FullbrightModule.class).effectiveGamma(wert)` ersetzen. **Nicht** `OptionInstance#set` aufrufen. In `frogclient.mixins.json` unter `client` eintragen.
- [ ] **Step 4: Registrieren.** `FrogKeys.register()` und `modules.register(new FullbrightModule())` in `onInitializeClient` (vor `config.load`). Bis die GUI existiert (Task 6), Fullbright zum Testen in `mod/run/config/frogclient.json` per `"fullbright": {"enabled": true}` einschalten.
- [ ] **Step 5: Manuell prüfen.** `./gradlew runClient`, Welt erstellen, Nacht/Höhle: mit Fullbright an alles hell; Steuerung zeigt Kategorie „FrogClient“ mit beiden Tasten. Gamma in `mod/run/options.txt` vor/nach Spielende vergleichen → identisch (Review Focus 4).
- [ ] **Step 6: Commit** `feat(mod): keybinds and fullbright`

---

### Task 4: Zoom

**Files:**
- Create: `mod/src/main/java/dev/frogclient/feature/zoom/{ZoomMath,ZoomModule}.java`, `mod/src/main/java/dev/frogclient/mixin/{GameRendererMixin,MouseHandlerMixin}.java`
- Modify: `FrogClient.java`, `frogclient.mixins.json`, Sprachdateien
- Test: `mod/src/test/java/dev/frogclient/feature/zoom/{ZoomMathTest,ZoomModuleTest}.java`

**Interfaces:**
- Consumes: `Module`, `SliderSetting`, `BoolSetting`, `FrogKeys.ZOOM`.
- Produces:
  - `final class ZoomMath` – `static final double SCROLL_STEP = 1.25`, `SCROLL_MIN = 1.5`, `SCROLL_MAX = 50.0`; `static double fov(double baseFov, double factor)` = `baseFov / factor`; `static double scroll(double factor, double wheelDelta)` = `factor * SCROLL_STEP^sign(delta)` (pro Event ein Schritt), geklemmt auf `[SCROLL_MIN, SCROLL_MAX]`; `static double sensitivityMultiplier(double factor)` = `1 / factor`.
  - `final class ZoomModule extends Module` – id `"zoom"`, `Category.RENDER`, default an. Settings: `factor` Slider 2–10, Schritt 0.5, Default 4; `scroll` Bool Default true; `smooth` Bool Default false. Zustand: `boolean isZooming()`, `double currentFactor()`. `void tick(boolean keyDown, boolean screenOpen, boolean windowFocused)` – Zoom aktiv genau dann, wenn Modul an ∧ `keyDown` ∧ `!screenOpen` ∧ `windowFocused`; beim Start `currentFactor = factor`, beim Ende Reset. `boolean onScroll(double delta)` – gibt `true` zurück (Event verbraucht), wenn gezoomt und `scroll` an. Smooth Camera: beim Zoom-Start bisherigen `Options.smoothCamera`-Wert merken und auf `true` setzen (wenn `smooth` an), beim Ende exakt zurücksetzen – über injizierbares `BooleanSupplier`/`Consumer<Boolean>`-Paar, damit testbar.

- [ ] **Step 1: Failing Tests:**

```java
// ZoomMathTest
@Test void fovDividesByFactor()        { assertEquals(17.5, ZoomMath.fov(70, 4), 1e-9); }
@Test void scrollStepsAndClamps()      { assertEquals(5.0, ZoomMath.scroll(4, 1), 1e-9);
                                         assertEquals(3.2, ZoomMath.scroll(4, -1), 1e-9);
                                         assertEquals(50.0, ZoomMath.scroll(49, 3), 1e-9);
                                         assertEquals(1.5, ZoomMath.scroll(1.6, -1), 1e-9); }
@Test void sensitivityShrinksWithZoom(){ assertEquals(0.25, ZoomMath.sensitivityMultiplier(4), 1e-9); }
// ZoomModuleTest
@Test void zoomsOnlyWhileKeyHeldWithoutScreen()  // tick(true,false,true) → zooming; tick(true,true,true) → nicht
@Test void openingScreenWhileHeldEndsZoomAndRestoresSmooth() // smooth an, vorher smoothCamera=false → während Zoom true, nach Screen öffnen false
@Test void focusLossEndsZoom()
@Test void scrollChangesFactorOnlyWhileZoomingAndResetsAfter()
@Test void scrollNotConsumedWhenScrollSettingOff()
@Test void disabledModuleNeverZooms()
```

- [ ] **Step 2: Tests → FAIL**, implementieren, Tests → PASS. Run: `./gradlew test`
- [ ] **Step 3: Verdrahtung.** `ClientTickEvents.END_CLIENT_TICK`: `zoom.tick(FrogKeys.ZOOM.isDown(), mc.screen != null, mc.isWindowActive())`. `GameRendererMixin`: Rückgabewert der FOV-Methode (1.21.x `GameRenderer#getFov`) bei `isZooming()` durch `ZoomMath.fov(wert, currentFactor())` ersetzen. `MouseHandlerMixin`: (a) in der Mausdrehung (1.21.x `MouseHandler#turnPlayer`) Sensitivität mit `sensitivityMultiplier` skalieren, (b) in `onScroll` vor Hotbar-Wechsel abbrechen, wenn `zoom.onScroll(delta)` `true` liefert. Mixin-Namen per genSources verifizieren.
- [ ] **Step 4: Manuell prüfen.** runClient: C halten zoomt, Mausrad ändert Zoom ohne Hotbar-Wechsel, Loslassen = normal; Chat öffnen und „c“ tippen zoomt nicht; C halten + Esc → Zoom endet. Hinweis: Vanilla belegt C für „Hotbar speichern“ (Kreativ) – Konflikt wird in der Steuerung rot angezeigt, ist akzeptiert (Nutzer-Entscheidung).
- [ ] **Step 5: Commit** `feat(mod): zoom`

---

### Task 5: HUD-Gerüst + FPS-Anzeige

**Files:**
- Create: `mod/src/main/java/dev/frogclient/hud/{HudPosition,HudElement,HudRenderer}.java`, `mod/src/main/java/dev/frogclient/feature/fps/FpsModule.java`
- Modify: `FrogClient.java`, Sprachdateien
- Test: `mod/src/test/java/dev/frogclient/hud/HudPositionTest.java`, `mod/src/test/java/dev/frogclient/feature/fps/FpsModuleTest.java`

**Interfaces:**
- Consumes: `Module`, `GuiState.hudPositions`, Settings.
- Produces:
  - `record HudPosition(double relX, double relY)` – `static final HudPosition DEFAULT_TOP_LEFT = new HudPosition(0.01, 0.01)`; `int[] toPixels(int screenW, int screenH, int elemW, int elemH)` – `x = round(relX*(screenW-elemW))`, analog y, geklemmt auf `[0, screen-elem]` (bei elem > screen → 0); `static HudPosition fromPixels(int x, int y, int screenW, int screenH, int elemW, int elemH)` – Umkehrung, geklemmt auf `[0,1]`, Division durch 0 → 0.
  - `interface HudElement` – `String hudId()`, `int width()`, `int height()` (unskaliert, inkl. Skalierung durch eigenes `scale`-Setting), `void render(GuiGraphics g, int x, int y)`, `boolean visible()`.
  - `final class HudRenderer` – `static void register(ModuleManager, GuiState)`: ein Fabric-HUD-Element (Fabric API `HudElementRegistry`, nach den Vanilla-Elementen) rendert alle sichtbaren `HudElement`-Module an `GuiState`-Position (Fallback `DEFAULT_TOP_LEFT`). Nicht sichtbar, wenn F1-HUD aus oder Debug-Screen offen.
  - `final class FpsModule extends Module implements HudElement` – id `"fps"`, `Category.HUD`, default an; Settings: `scale` Slider 0.5–3, Schritt 0.25, Default 1; `color` Color Default `0xFFFFFFFF`; `background` Bool Default false. `String text(int fps)` → `"<fps> FPS"`, cached: gleicher fps-Wert → identische String-Instanz (keine Allokation pro Frame).

- [ ] **Step 1: Failing Tests:**

```java
@Test void topLeftDefault()                 // DEFAULT_TOP_LEFT.toPixels(1920,1080,50,10) → [19, 11]
@Test void clampsWhenScreenShrinks()        // new HudPosition(1,1).toPixels(800,600,50,10) → [750, 590]
@Test void elementLargerThanScreen()        // toPixels(40,40,50,10) → x == 0
@Test void roundTripPixels()                // fromPixels(...).toPixels(...) == Ursprung (±1)
@Test void fromPixelsZeroSpace()            // screenW == elemW → relX 0, keine Exception
@Test void fpsTextCachedPerValue()          // assertSame(m.text(144), m.text(144)); assertEquals("60 FPS", m.text(60))
```

- [ ] **Step 2: Tests → FAIL**, implementieren, Tests → PASS.
- [ ] **Step 3: Rendering.** Text mit `GuiGraphics#drawString` (Schatten an), skaliert über Pose-Stack um `scale`; Hintergrund: halbtransparentes Rechteck `0x80000000` mit 2px Padding. FPS-Wert aus `Minecraft#getFps()`.
- [ ] **Step 4: Manuell prüfen.** runClient: „xx FPS“ oben links; Fenstergröße/GUI-Scale ändern → bleibt sichtbar (Review Focus 2).
- [ ] **Step 5: Commit** `feat(mod): HUD framework and FPS display`

---

### Task 6: Client-GUI mit zwei Layouts

**Files:**
- Create: `mod/src/main/java/dev/frogclient/gui/{ClientScreen,GuiLayout,PanelLayout,WindowLayout}.java`
- Create: `mod/src/main/java/dev/frogclient/gui/widget/{SettingWidget,ToggleWidget,SliderWidget,EnumWidget,ColorWidget,WidgetFactory}.java`
- Create: `mod/src/main/java/dev/frogclient/gui/PanelMath.java`
- Modify: `FrogClient.java`, Sprachdateien
- Test: `mod/src/test/java/dev/frogclient/gui/PanelMathTest.java`

**Interfaces:**
- Consumes: `ModuleManager`, `Module`, alle `Setting`-Typen, `GuiState`, `FrogKeys.OPEN_GUI`, `FrogClient.saveConfig()`.
- Produces:
  - `interface SettingWidget` – `int height()`, `void render(GuiGraphics g, int x, int y, int width, int mouseX, int mouseY)`, `boolean mouseClicked(double mx, double my, int button)`, `boolean mouseDragged(double mx, double my)`, `void mouseReleased()`.
  - `final class WidgetFactory` – `static SettingWidget create(Setting<?> s)` per `switch` über sealed `Setting`: Toggle (Schalter in Akzentfarbe), Slider (Ziehen, Wert-Anzeige mit Schrittweite), Enum (Klick = `next()`), Color (Farbfeld, Klick = `next()`).
  - `interface GuiLayout` – `void init(ClientScreen screen, int width, int height)`, `void render(GuiGraphics g, int mouseX, int mouseY, float delta)`, `boolean mouseClicked(double, double, int)`, `boolean mouseDragged(double, double, int, double, double)`, `boolean mouseReleased(double, double, int)`, `boolean mouseScrolled(double, double, double)`.
  - `final class PanelMath` – `static int[] clamp(int x, int y, int panelW, int panelH, int screenW, int screenH)`; `static int[][] autoArrange(int count, int panelW, int gap, int startX, int startY)` (nebeneinander, Abstand `gap`).
  - `PanelLayout` (A): pro `Category` ein Panel (Breite 120 GUI-px, Kopf in Akzentfarbe mit Kategorienamen); Kopf ziehen verschiebt; Position → `GuiState.panelPositions` (fehlend → `autoArrange`). Modulzeile: Linksklick = `toggle()`, Rechtsklick klappt Settings-Widgets darunter auf/zu.
  - `WindowLayout` (B): zentriertes Fenster 82 % × 76 % des Screens, links Kategorien (Logo „🐸 FrogClient“ bzw. Text „FrogClient“ in Akzentfarbe), rechts Modul-Kacheln im 2-Spalten-Raster mit Schalter; Klick auf Kachel (außerhalb Schalter) öffnet Settings-Ansicht des Moduls mit Zurück-Knopf; scrollbar bei Überlauf.
  - `ClientScreen extends Screen` – `isPauseScreen() = false`, abgedunkelter Hintergrund `0x73000000`; oben rechts Knopf „Layout: Panels/Fenster“ (wechselt `GuiState.layout` und re-initialisiert) und Knopf „HUD bearbeiten“ (öffnet `HudEditorScreen`, Task 7); `onClose()` → `FrogClient.saveConfig()`.
  - Öffnen: in `END_CLIENT_TICK` `while (FrogKeys.OPEN_GUI.consumeClick())` → `mc.setScreen(new ClientScreen())`, nur wenn `mc.screen == null`.
  - Kategorie `SETTINGS` hat in v1 keine Module; Panel/Eintrag wird nur angezeigt, wenn Module darin existieren.

- [ ] **Step 1: Failing Tests (`PanelMathTest`):**

```java
@Test void clampKeepsPanelOnScreen()      // clamp(900, 500, 120, 80, 854, 480) → [734, 400]
@Test void clampNegative()                // clamp(-50, -10, 120, 80, 854, 480) → [0, 0]
@Test void autoArrangeSideBySide()        // autoArrange(3, 120, 10, 10, 14) → [[10,14],[140,14],[270,14]]
```

- [ ] **Step 2: Tests → FAIL**, `PanelMath` implementieren, Tests → PASS.
- [ ] **Step 3: Widgets + beide Layouts + `ClientScreen` implementieren.** Panels werden in `init` (auch nach Resize) per `PanelMath.clamp` auf den Bildschirm geholt (Review Focus 2). Alle Texte über Sprachschlüssel (`frogclient.gui.layout.panels`, `…window`, `…edit_hud`, `…back`, Modul-/Setting-/Kategorienamen) in `en_us` und `de_de`.
- [ ] **Step 4: Manuell prüfen.** runClient: Rechte Umschalt öffnet GUI; Layout-Knopf wechselt A↔B; in beiden: Fullbright/Zoom/FPS umschalten, Zoom-Stärke, FPS-Farbe/Größe/Hintergrund ändern → wirkt sofort. Panel verschieben, GUI schließen, Spiel neu starten → Layout, Panel-Positionen, Werte erhalten. Fenster stark verkleinern → Panels bleiben sichtbar.
- [ ] **Step 5: Commit** `feat(mod): client GUI with panel and window layouts`

---

### Task 7: HUD-Editor

**Files:**
- Create: `mod/src/main/java/dev/frogclient/hud/HudEditorScreen.java`
- Modify: `ClientScreen.java` (Knopf verdrahten), Sprachdateien

**Interfaces:**
- Consumes: `HudElement`, `HudPosition`, `GuiState.hudPositions`, `ModuleManager`, `FrogClient.saveConfig()`.
- Produces: `HudEditorScreen(Screen parent) extends Screen` – zeigt alle aktivierten `HudElement`s an ihren Positionen mit Rahmen in Akzentfarbe; Linksklick+Ziehen verschiebt (Position via `HudPosition.fromPixels`, live geklemmt); Rechtsklick auf Element → zurück auf `DEFAULT_TOP_LEFT`; Hinweistext unten mittig („Ziehen zum Verschieben · Rechtsklick: zurücksetzen · Esc: fertig“); `onClose()` speichert und kehrt zu `parent` zurück.

- [ ] **Step 1: Implementieren** (Positionslogik ist durch `HudPositionTest` aus Task 5 abgedeckt).
- [ ] **Step 2: Manuell prüfen.** FPS-Anzeige in jede Ecke ziehen, Esc, Spiel neu starten → Position erhalten; Fenstergröße ändern → Element bleibt in derselben relativen Ecke und sichtbar.
- [ ] **Step 3: Commit** `feat(mod): HUD editor`

---

### Task 8: Abnahme gegen Spec

**Files:**
- Modify: `README.md` (Abschnitt „Building the mod“: `cd mod && ./gradlew build`, Java 25 nötig)

- [ ] **Step 1: Volltest.** Run: `./gradlew clean test build` → BUILD SUCCESSFUL, alle Tests grün.
- [ ] **Step 2: Erfolgskriterium 6** (Spec §7) komplett manuell durchspielen: GUI per Rechte Umschalt, Layout-Umschalter, Zoom/FPS/Fullbright wie beschrieben, Einstellungen überleben Neustart.
- [ ] **Step 3: Erfolgskriterium 7 (Performance).** Da der eigene Launcher noch nicht existiert: in einer Prism-/Vanilla-Launcher-Instanz 26.3 + Fabric 0.19.5 + Fabric API + Sodium, gleiche Welt/Seed/Position, Render-Distanz 12, je 30 s FPS-Mittel messen: (a) mit `frogclient-0.1.0.jar`, alle Module aus, (b) ohne die JAR. Abweichung < 2 % = bestanden. Messwerte im Commit-Text festhalten. Kann der Nutzer übernehmen, falls kein Prism installiert ist.
- [ ] **Step 4: `config-dir`-Property prüfen.** `runClient` mit `-Dfrogclient.configDir=<tempdir>` (in `build.gradle` `loom.runs.client.vmArg` temporär) → `frogclient.json` entsteht dort.
- [ ] **Step 5: Commit** `docs: build instructions for mod` und Push nach Rücksprache mit dem Nutzer.
