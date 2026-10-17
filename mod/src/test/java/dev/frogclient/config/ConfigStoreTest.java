package dev.frogclient.config;

import dev.frogclient.module.Category;
import dev.frogclient.module.Module;
import dev.frogclient.module.ModuleManager;
import dev.frogclient.setting.SliderSetting;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

import java.nio.file.Files;
import java.nio.file.Path;

import static org.junit.jupiter.api.Assertions.*;

class ConfigStoreTest {
	static final class Zoom extends Module {
		final SliderSetting factor = add(new SliderSetting("factor", "k", 2, 10, 0.5, 4));

		Zoom() {
			super("zoom", Category.RENDER, true);
		}
	}

	private static ModuleManager modules(Zoom z) {
		var mm = new ModuleManager();
		mm.register(z);
		return mm;
	}

	@Test
	void saveThenLoadRestoresEverything(@TempDir Path dir) {
		var z = new Zoom();
		z.setEnabled(false);
		z.factor.set(7.5);
		var gui = new GuiState();
		gui.setLayout(GuiState.Layout.WINDOW);
		gui.panelPositions().put(Category.RENDER, new int[]{30, 40});
		gui.hudPositions().put("fps", new double[]{0.5, 0.25});
		new ConfigStore(dir).save(modules(z), gui);

		var z2 = new Zoom();
		var gui2 = new GuiState();
		new ConfigStore(dir).load(modules(z2), gui2);
		assertFalse(z2.isEnabled());
		assertEquals(7.5, z2.factor.get());
		assertEquals(GuiState.Layout.WINDOW, gui2.layout());
		assertArrayEquals(new int[]{30, 40}, gui2.panelPositions().get(Category.RENDER));
		assertArrayEquals(new double[]{0.5, 0.25}, gui2.hudPositions().get("fps"));
	}

	@Test
	void missingFileGivesDefaults(@TempDir Path dir) {
		var z = new Zoom();
		var gui = new GuiState();
		var store = new ConfigStore(dir);
		store.load(modules(z), gui);
		assertTrue(z.isEnabled());
		assertEquals(4.0, z.factor.get());
		assertEquals(GuiState.Layout.PANELS, gui.layout());
		assertFalse(Files.exists(store.file()));
	}

	@Test
	void corruptFileGivesDefaultsAndBackup(@TempDir Path dir) throws Exception {
		var store = new ConfigStore(dir);
		Files.writeString(store.file(), "{nicht json");
		var z = new Zoom();
		var gui = new GuiState();
		store.load(modules(z), gui);
		assertTrue(z.isEnabled());
		assertEquals(4.0, z.factor.get());
		Path bak = dir.resolve("frogclient.json.bak");
		assertTrue(Files.exists(bak));
		assertEquals("{nicht json", Files.readString(bak));
	}

	@Test
	void partiallyInvalidKeepsValidParts(@TempDir Path dir) throws Exception {
		var store = new ConfigStore(dir);
		Files.writeString(store.file(), """
				{ "version": 1,
				  "gui": { "layout": "WINDOW", "panels": { "RENDER": "kaputt" } },
				  "hud": { "fps": [0.2] },
				  "modules": { "zoom": { "enabled": "ja", "settings": { "factor": "x" } } } }
				""");
		var z = new Zoom();
		var gui = new GuiState();
		store.load(modules(z), gui);
		assertEquals(GuiState.Layout.WINDOW, gui.layout());
		assertTrue(z.isEnabled());
		assertEquals(4.0, z.factor.get());
		assertFalse(gui.panelPositions().containsKey(Category.RENDER));
		assertFalse(gui.hudPositions().containsKey("fps"));
	}

	@Test
	void unknownModulesAndKeysIgnored(@TempDir Path dir) throws Exception {
		var store = new ConfigStore(dir);
		Files.writeString(store.file(), """
				{ "version": 1, "extra": 5,
				  "gui": { "panels": { "NOPE": [1, 2] } },
				  "modules": { "ghost": { "enabled": true },
				               "zoom": { "enabled": false, "settings": { "nope": 1, "factor": 6 } } } }
				""");
		var z = new Zoom();
		store.load(modules(z), new GuiState());
		assertFalse(z.isEnabled());
		assertEquals(6.0, z.factor.get());
	}

	@Test
	void saveIsAtomic(@TempDir Path dir) throws Exception {
		var store = new ConfigStore(dir);
		store.save(modules(new Zoom()), new GuiState());
		store.save(modules(new Zoom()), new GuiState());
		try (var files = Files.list(dir)) {
			assertTrue(files.noneMatch(p -> p.getFileName().toString().endsWith(".tmp")));
		}
		assertTrue(Files.readString(store.file()).contains("\"zoom\""));
	}

	@Test
	void saveIfChangedKeepsOtherInstancesWriteWhenNothingChanged(@TempDir Path dir) throws Exception {
		var store = new ConfigStore(dir);
		var mine = new Zoom();
		store.load(modules(mine), new GuiState());

		// Another instance saves a change while this one is running.
		var other = new Zoom();
		other.factor.set(8.0);
		new ConfigStore(dir).save(modules(other), new GuiState());

		store.saveIfChanged(modules(mine), new GuiState());
		var check = new Zoom();
		new ConfigStore(dir).load(modules(check), new GuiState());
		assertEquals(8.0, check.factor.get());
	}

	@Test
	void saveIfChangedWritesAfterLocalChange(@TempDir Path dir) {
		var store = new ConfigStore(dir);
		var z = new Zoom();
		var gui = new GuiState();
		store.load(modules(z), gui);
		z.factor.set(6.0);
		store.saveIfChanged(modules(z), gui);
		var check = new Zoom();
		new ConfigStore(dir).load(modules(check), new GuiState());
		assertEquals(6.0, check.factor.get());
	}

	@Test
	void concurrentSavesFromTwoInstancesNeverCorruptFile(@TempDir Path dir) throws Exception {
		Runnable writer = () -> {
			var store = new ConfigStore(dir);
			for (int i = 0; i < 300; i++) {
				var z = new Zoom();
				z.factor.set(2.0 + (i % 16) * 0.5);
				store.save(modules(z), new GuiState());
			}
		};
		Thread a = new Thread(writer);
		Thread b = new Thread(writer);
		a.start();
		b.start();
		a.join();
		b.join();
		var z = new Zoom();
		new ConfigStore(dir).load(modules(z), new GuiState());
		assertFalse(Files.exists(dir.resolve("frogclient.json.bak")), "config was corrupted by concurrent writers");
		try (var files = Files.list(dir)) {
			assertTrue(files.noneMatch(p -> p.getFileName().toString().endsWith(".tmp")));
		}
	}

	@Test
	void resolveDirUsesSystemProperty(@TempDir Path dir) {
		System.setProperty(ConfigStore.DIR_PROPERTY, dir.toString());
		try {
			assertEquals(dir, ConfigStore.resolveDir());
		} finally {
			System.clearProperty(ConfigStore.DIR_PROPERTY);
		}
	}
}
