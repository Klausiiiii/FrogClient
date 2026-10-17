package dev.frogclient.config;

import com.google.gson.Gson;
import com.google.gson.GsonBuilder;
import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import dev.frogclient.FrogClient;
import dev.frogclient.module.Category;
import dev.frogclient.module.Module;
import dev.frogclient.module.ModuleManager;
import dev.frogclient.setting.Setting;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.AtomicMoveNotSupportedException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardCopyOption;
import java.util.Map;

/**
 * Reads and writes {@code frogclient.json}. Loading never throws: every section that
 * fails to parse keeps its defaults. Writing goes through a temp file and a move.
 */
public final class ConfigStore {
	public static final String DIR_PROPERTY = "frogclient.configDir";
	private static final int VERSION = 1;
	private static final Gson GSON = new GsonBuilder().setPrettyPrinting().create();

	private final Path dir;

	public ConfigStore(Path dir) {
		this.dir = dir;
	}

	public static Path resolveDir() {
		String prop = System.getProperty(DIR_PROPERTY);
		if (prop != null && !prop.isBlank()) return Path.of(prop);
		return net.fabricmc.loader.api.FabricLoader.getInstance().getConfigDir();
	}

	public Path file() {
		return dir.resolve("frogclient.json");
	}

	public void load(ModuleManager modules, GuiState gui) {
		Path file = file();
		if (!Files.exists(file)) return;

		JsonObject root;
		try {
			root = JsonParser.parseString(Files.readString(file, StandardCharsets.UTF_8)).getAsJsonObject();
		} catch (Exception e) {
			FrogClient.LOG.warn("frogclient.json is unreadable, using defaults (backup: frogclient.json.bak)", e);
			try {
				Files.copy(file, dir.resolve("frogclient.json.bak"), StandardCopyOption.REPLACE_EXISTING);
			} catch (IOException io) {
				FrogClient.LOG.warn("Could not back up broken config", io);
			}
			return;
		}

		section("gui", () -> loadGui(object(root, "gui"), gui));
		section("hud", () -> loadHud(object(root, "hud"), gui));
		section("modules", () -> loadModules(object(root, "modules"), modules));
	}

	private static void loadGui(JsonObject json, GuiState gui) {
		if (json == null) return;
		section("gui.layout", () -> gui.setLayout(GuiState.Layout.valueOf(json.get("layout").getAsString())));
		JsonObject panels = object(json, "panels");
		if (panels == null) return;
		for (Map.Entry<String, JsonElement> e : panels.entrySet()) {
			section("gui.panels." + e.getKey(), () -> {
				Category category = Category.valueOf(e.getKey());
				JsonArray a = e.getValue().getAsJsonArray();
				if (a.size() != 2) return;
				gui.panelPositions().put(category, new int[]{a.get(0).getAsInt(), a.get(1).getAsInt()});
			});
		}
	}

	private static void loadHud(JsonObject json, GuiState gui) {
		if (json == null) return;
		for (Map.Entry<String, JsonElement> e : json.entrySet()) {
			section("hud." + e.getKey(), () -> {
				JsonArray a = e.getValue().getAsJsonArray();
				if (a.size() != 2) return;
				gui.hudPositions().put(e.getKey(), new double[]{a.get(0).getAsDouble(), a.get(1).getAsDouble()});
			});
		}
	}

	private static void loadModules(JsonObject json, ModuleManager modules) {
		if (json == null) return;
		for (Module module : modules.all()) {
			JsonObject m = object(json, module.id());
			if (m == null) continue;
			section("modules." + module.id() + ".enabled", () -> {
				JsonElement enabled = m.get("enabled");
				if (enabled != null && enabled.isJsonPrimitive() && enabled.getAsJsonPrimitive().isBoolean()) {
					module.setEnabled(enabled.getAsBoolean());
				}
			});
			JsonObject settings = object(m, "settings");
			if (settings == null) continue;
			for (Setting<?> setting : module.settings()) {
				if (settings.has(setting.id())) setting.fromJson(settings.get(setting.id()));
			}
		}
	}

	public void save(ModuleManager modules, GuiState gui) {
		JsonObject root = new JsonObject();
		root.addProperty("version", VERSION);

		JsonObject guiJson = new JsonObject();
		guiJson.addProperty("layout", gui.layout().name());
		JsonObject panels = new JsonObject();
		gui.panelPositions().forEach((c, p) -> panels.add(c.name(), array(p[0], p[1])));
		guiJson.add("panels", panels);
		root.add("gui", guiJson);

		JsonObject hud = new JsonObject();
		gui.hudPositions().forEach((id, p) -> hud.add(id, array(p[0], p[1])));
		root.add("hud", hud);

		JsonObject modulesJson = new JsonObject();
		for (Module module : modules.all()) {
			JsonObject m = new JsonObject();
			m.addProperty("enabled", module.isEnabled());
			JsonObject settings = new JsonObject();
			for (Setting<?> setting : module.settings()) settings.add(setting.id(), setting.toJson());
			m.add("settings", settings);
			modulesJson.add(module.id(), m);
		}
		root.add("modules", modulesJson);

		Path file = file();
		Path tmp = dir.resolve("frogclient.json.tmp");
		try {
			Files.createDirectories(dir);
			Files.writeString(tmp, GSON.toJson(root), StandardCharsets.UTF_8);
			try {
				Files.move(tmp, file, StandardCopyOption.ATOMIC_MOVE, StandardCopyOption.REPLACE_EXISTING);
			} catch (AtomicMoveNotSupportedException e) {
				Files.move(tmp, file, StandardCopyOption.REPLACE_EXISTING);
			}
		} catch (IOException e) {
			FrogClient.LOG.error("Could not save frogclient.json", e);
		}
	}

	private static JsonObject object(JsonObject parent, String key) {
		JsonElement e = parent.get(key);
		return e != null && e.isJsonObject() ? e.getAsJsonObject() : null;
	}

	private static JsonArray array(Number a, Number b) {
		JsonArray arr = new JsonArray();
		arr.add(a);
		arr.add(b);
		return arr;
	}

	private static void section(String name, Runnable action) {
		try {
			action.run();
		} catch (RuntimeException e) {
			FrogClient.LOG.warn("Ignoring invalid config entry '{}': {}", name, e.toString());
		}
	}
}
