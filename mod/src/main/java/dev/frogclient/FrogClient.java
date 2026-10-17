package dev.frogclient;

import net.fabricmc.api.ClientModInitializer;
import net.fabricmc.loader.api.FabricLoader;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

public final class FrogClient implements ClientModInitializer {
	public static final String MOD_ID = "frogclient";
	public static final Logger LOG = LoggerFactory.getLogger("FrogClient");

	@Override
	public void onInitializeClient() {
		String version = FabricLoader.getInstance().getModContainer(MOD_ID)
				.map(c -> c.getMetadata().getVersion().getFriendlyString())
				.orElse("?");
		LOG.info("FrogClient {} geladen", version);
	}
}
