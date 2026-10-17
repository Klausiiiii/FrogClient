package dev.frogclient.module;

import java.util.ArrayList;
import java.util.List;

public final class ModuleManager {
	private final List<Module> modules = new ArrayList<>();

	public <M extends Module> M register(M module) {
		modules.add(module);
		return module;
	}

	public List<Module> all() {
		return List.copyOf(modules);
	}

	public List<Module> byCategory(Category category) {
		return modules.stream().filter(m -> m.category() == category).toList();
	}

	public <M extends Module> M get(Class<M> type) {
		for (Module m : modules) {
			if (type.isInstance(m)) return type.cast(m);
		}
		throw new IllegalStateException("Module not registered: " + type.getSimpleName());
	}
}
