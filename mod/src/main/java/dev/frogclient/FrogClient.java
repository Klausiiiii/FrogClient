package dev.frogclient;

import dev.frogclient.config.ConfigStore;
import dev.frogclient.config.GuiState;
import dev.frogclient.feature.FullbrightModule;
import dev.frogclient.module.ModuleManager;
import net.fabricmc.api.ClientModInitializer;
import net.fabricmc.fabric.api.client.event.lifecycle.v1.ClientLifecycleEvents;
import net.fabricmc.loader.api.FabricLoader;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

public final class FrogClient implements ClientModInitializer {
	public static final String MOD_ID = "frogclient";
	public static final Logger LOG = LoggerFactory.getLogger("FrogClient");

	public static final ModuleManager modules = new ModuleManager();
	public static final GuiState gui = new GuiState();
	private static ConfigStore config;

	@Override
	public void onInitializeClient() {
		String version = FabricLoader.getInstance().getModContainer(MOD_ID)
				.map(c -> c.getMetadata().getVersion().getFriendlyString())
				.orElse("?");

		FrogKeys.register();
		modules.register(new FullbrightModule());

		config = new ConfigStore(ConfigStore.resolveDir());
		config.load(modules, gui);
		ClientLifecycleEvents.CLIENT_STOPPING.register(client -> saveConfig());

		LOG.info("FrogClient {} geladen", version);
	}

	public static void saveConfig() {
		if (config != null) config.save(modules, gui);
	}
}
