package dev.frogclient.feature;

import dev.frogclient.module.Category;
import dev.frogclient.module.Module;

/** Full brightness by overriding the gamma value the lightmap reads; options.txt stays untouched. */
public final class FullbrightModule extends Module {
	public static final double GAMMA_OVERRIDE = 16.0;

	public FullbrightModule() {
		super("fullbright", Category.RENDER, false);
	}

	public double effectiveGamma(double optionGamma) {
		return isEnabled() ? GAMMA_OVERRIDE : optionGamma;
	}
}
