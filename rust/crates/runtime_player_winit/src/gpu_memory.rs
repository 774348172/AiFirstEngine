//! Read-only budget query on the same live Vulkan device. No adapter enumeration,
//! extension enablement, reservations, or configured-memory subtraction.
use ash::vk;
use engine_runtime::resource_memory::{GpuLocalHeapBudget, GpuMemoryBudgetSample};

pub(crate) fn sample(device: &wgpu::Device) -> Option<GpuMemoryBudgetSample> {
    // SAFETY: the guard borrows the live wgpu device; we neither destroy nor mutate it.
    let hal = unsafe { device.as_hal::<wgpu::hal::api::Vulkan>() }?;
    if !hal
        .enabled_device_extensions()
        .contains(&ash::ext::memory_budget::NAME)
        || hal.shared_instance().instance_api_version() < vk::API_VERSION_1_1
    {
        return None;
    }
    let instance = hal.shared_instance().raw_instance();
    let physical = hal.raw_physical_device();
    let mut driver = vk::PhysicalDeviceMemoryBudgetPropertiesEXT::default();
    let mut properties = vk::PhysicalDeviceMemoryProperties2::default().push_next(&mut driver);
    let mut identity = vk::PhysicalDeviceIDProperties::default();
    let mut device_properties = vk::PhysicalDeviceProperties2::default().push_next(&mut identity);
    // SAFETY: supported extension/core API, same borrowed physical device and valid
    // output chains for these synchronous read-only Vulkan calls.
    unsafe {
        instance.get_physical_device_memory_properties2(physical, &mut properties);
        instance.get_physical_device_properties2(physical, &mut device_properties);
    }
    let memory = properties.memory_properties;
    let local_heaps = memory.memory_heaps[..memory.memory_heap_count as usize]
        .iter()
        .enumerate()
        .filter(|(_, heap)| heap.flags.contains(vk::MemoryHeapFlags::DEVICE_LOCAL))
        .map(|(index, heap)| GpuLocalHeapBudget {
            heap_index: index as u32,
            heap_size: heap.size,
            budget: driver.heap_budget[index],
            usage: driver.heap_usage[index],
        })
        .collect();
    let sample = GpuMemoryBudgetSample {
        source: "VK_EXT_memory_budget".into(),
        adapter_uuid: identity
            .device_uuid
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect(),
        local_heaps,
    };
    sample.available_bytes()?;
    Some(sample)
}
