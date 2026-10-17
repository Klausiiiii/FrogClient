package dev.frogclient.feature.zoom;

public final class ZoomMath {
	public static final double SCROLL_STEP = 1.25;
	public static final double SCROLL_MIN = 1.5;
	public static final double SCROLL_MAX = 50.0;

	private ZoomMath() {
	}

	public static double fov(double baseFov, double factor) {
		return baseFov / factor;
	}

	/** One step per scroll event, direction from the sign of the wheel delta. */
	public static double scroll(double factor, double wheelDelta) {
		double next = factor * Math.pow(SCROLL_STEP, Math.signum(wheelDelta));
		return Math.max(SCROLL_MIN, Math.min(SCROLL_MAX, next));
	}

	public static double sensitivityMultiplier(double factor) {
		return 1.0 / factor;
	}
}
