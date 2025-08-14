#!/vendor/bin/sh

#
# Copyright (C) 2024-2025 Zexshia
#
# Licensed under the Apache License, Version 2.0 (the "License");
# you may not use this file except in compliance with the License.
# You may obtain a copy of the License at
#
#      http://www.apache.org/licenses/LICENSE-2.0
#
# Unless required by applicable law or agreed to in writing, software
# distributed under the License is distributed on an "AS IS" BASIS,
# WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
# See the License for the specific language governing permissions and
# limitations under the License.
#

# shellcheck disable=SC2013
# Add for debug prop
DEBUG_LOG=$(getprop persist.sys.azenith-debug)

AZLog() {
    if [ "$DEBUG_LOG" = "true" ]; then
        log -p i -t "AZenith" "$1"
    fi
}

# fix dumpsys
export PATH="/product/bin:/apex/com.android.runtime/bin:/apex/com.android.art/bin:/system_ext/bin:/system/bin:/system/xbin:/odm/bin:/vendor/bin:/vendor/xbin"
AZLog "Runtime PATH was set to: $PATH"

DEFAULT_GOV_FILE="/sdcard/config/AZenithDefaultGov"

zeshia() {
    local value="$1"
    local path="$2"

    # Func Called log
    AZLog "Attempting to set '$path' to '$value'"
    if [ ! -e "$path" ]; then
        return 1
    fi

    # Ensure writable
    if [ ! -w "$path" ]; then
        if chmod 0666 "$path" 2>/dev/null; then
            AZLog "Made '$path' writable"
        else
            AZLog "FAILED: Cannot make '$path' writable"
            return 1
        fi
    fi

    # First write attempt
    echo "$value" >"$path" 2>/dev/null
    local current
    current="$(cat "$path" 2>/dev/null)"

    if [ "$current" = "$value" ]; then
        AZLog "SUCCESS: '$path' set to '$current'"
    else
        # Retry once
        AZLog "Retrying write to '$path'"
        echo "$value" >"$path" 2>/dev/null
        current="$(cat "$path" 2>/dev/null)"

        if [ "$current" = "$value" ]; then
            AZLog "SUCCESS on retry: '$path' set to '$current'"
        else
            AZLog "FAILED: Could not set '$path' to '$value' (current: '$current')"
        fi
    fi
}

zeshiax() {
    local value="$1"
    local path="$2"

    # Log the action before attempting it
    AZLog "Setting(x) '$path' to '$value'"

    if [ ! -e "$path" ]; then
        return
    fi
    if [ ! -w "$path" ] && ! chmod 644 "$path" 2>/dev/null; then
        return
    fi

    echo "$value" >"$path" 2>/dev/null
    local current
    current="$(cat "$path" 2>/dev/null)"

    if [ "$current" != "$value" ]; then
        echo "$value" >"$path" 2>/dev/null
        # No further logging—silent retry
    fi
}


which_maxfreq() {
	tr ' ' '\n' <"$1" | sort -nr | head -n 1
}

which_minfreq() {
	tr ' ' '\n' <"$1" | /vendor/bin/grep -v '^[[:space:]]*$' | sort -n | head -n 1
}

which_midfreq() {
	total_opp=$(wc -w <"$1")
	mid_opp=$(((total_opp + 1) / 2))
	tr ' ' '\n' <"$1" | /vendor/bin/grep -v '^[[:space:]]*$' | sort -nr | head -n $mid_opp | tail -n 1
}
cpufreq_ppm_max_perf() {
	cluster=-1
	for path in /sys/devices/system/cpu/cpufreq/policy*; do
		((cluster++))
		cpu_maxfreq=$(<"$path/cpuinfo_max_freq")
		zeshia "$cluster $cpu_maxfreq" /proc/ppm/policy/hard_userlimit_max_cpu_freq

		[ $LITE_MODE -eq 1 ] && {
			cpu_midfreq=$(which_midfreq "$path/scaling_available_frequencies")
			zeshia "$cluster $cpu_midfreq" /proc/ppm/policy/hard_userlimit_min_cpu_freq
			continue
		}

		zeshia "$cluster $cpu_maxfreq" /proc/ppm/policy/hard_userlimit_min_cpu_freq
	done
}

cpufreq_max_perf() {
	for path in /sys/devices/system/cpu/*/cpufreq; do
		cpu_maxfreq=$(<"$path/cpuinfo_max_freq")
		zeshia "$cpu_maxfreq" "$path/scaling_max_freq"

		[ $LITE_MODE -eq 1 ] && {
			cpu_midfreq=$(which_midfreq "$path/scaling_available_frequencies")
			zeshia "$cpu_midfreq" "$path/scaling_min_freq"
			continue
		}

	    zeshia "$cpu_maxfreq" "$path/scaling_min_freq"
	done
	chmod -f 444 /sys/devices/system/cpu/cpufreq/policy*/scaling_*_freq
}

cpufreq_ppm_unlock() {
	cluster=0
	for path in /sys/devices/system/cpu/cpufreq/policy*; do
		cpu_maxfreq=$(<"$path/cpuinfo_max_freq")
		cpu_minfreq=$(<"$path/cpuinfo_min_freq")
		zeshiax "$cluster $cpu_maxfreq" /proc/ppm/policy/hard_userlimit_max_cpu_freq
		zeshiax "$cluster $cpu_minfreq" /proc/ppm/policy/hard_userlimit_min_cpu_freq
		((cluster++))
	done
}

cpufreq_unlock() {
	for path in /sys/devices/system/cpu/*/cpufreq; do
		cpu_maxfreq=$(<"$path/cpuinfo_max_freq")
		cpu_minfreq=$(<"$path/cpuinfo_min_freq")
		zeshiax "$cpu_maxfreq" "$path/scaling_max_freq"
		zeshiax "$cpu_minfreq" "$path/scaling_min_freq"
	done
	chmod -f 644 /sys/devices/system/cpu/cpufreq/policy*/scaling_*_freq
}

devfreq_max_perf() {
	[ ! -f "$1/available_frequencies" ] && return 1
	max_freq=$(which_maxfreq "$1/available_frequencies")
	zeshia "$max_freq" "$1/max_freq"
	zeshia "$max_freq" "$1/min_freq"
}

devfreq_mid_perf() {
	[ ! -f "$1/available_frequencies" ] && return 1
	max_freq=$(which_maxfreq "$1/available_frequencies")
	mid_freq=$(which_midfreq "$1/available_frequencies")
	zeshia "$max_freq" "$1/max_freq"
	zeshia "$mid_freq" "$1/min_freq"
}

devfreq_unlock() {
	[ ! -f "$1/available_frequencies" ] && return 1
	max_freq=$(which_maxfreq "$1/available_frequencies")
	min_freq=$(which_minfreq "$1/available_frequencies")
	zeshiax "$max_freq" "$1/max_freq"
	zeshiax "$min_freq" "$1/min_freq"
}

devfreq_min_perf() {
	[ ! -f "$1/available_frequencies" ] && return 1
	freq=$(which_minfreq "$1/available_frequencies")
	zeshia "$freq" "$1/min_freq"
	zeshia "$freq" "$1/max_freq"
}

qcom_cpudcvs_max_perf() {
	[ ! -f "$1/available_frequencies" ] && return 1
	freq=$(which_maxfreq "$1/available_frequencies")
	zeshia "$freq" "$1/hw_max_freq"
	zeshia "$freq" "$1/hw_min_freq"
}

qcom_cpudcvs_mid_perf() {
	[ ! -f "$1/available_frequencies" ] && return 1
	max_freq=$(which_maxfreq "$1/available_frequencies")
	mid_freq=$(which_midfreq "$1/available_frequencies")
	zeshia "$max_freq" "$1/hw_max_freq"
	zeshia "$mid_freq" "$1/hw_min_freq"
}

qcom_cpudcvs_unlock() {
	[ ! -f "$1/available_frequencies" ] && return 1
	max_freq=$(which_maxfreq "$1/available_frequencies")
	min_freq=$(which_minfreq "$1/available_frequencies")
	zeshiax "$max_freq" "$1/hw_max_freq"
	zeshiax "$min_freq" "$1/hw_min_freq"
}

qcom_cpudcvs_min_perf() {
	[ ! -f "$1/available_frequencies" ] && return 1
	freq=$(which_minfreq "$1/available_frequencies")
	zeshia "$freq" "$1/hw_min_freq"
	zeshia "$freq" "$1/hw_max_freq"
}

setgov() {
    AZLog "Setting CPU governor to '$1'"
	chmod 644 /sys/devices/system/cpu/cpu*/cpufreq/scaling_governor
	echo "$1" | tee /sys/devices/system/cpu/cpu*/cpufreq/scaling_governor >/dev/null
	chmod 444 /sys/devices/system/cpu/cpu*/cpufreq/scaling_governor
	chmod 444 /sys/devices/system/cpu/cpufreq/policy*/scaling_governor
}

sync

###############################################
# # # # # # #  MEDIATEK BALANCE # # # # # # #
###############################################
mediatek_balance() {
    # PPM Settings
    if [ -d /proc/ppm ]; then
        if [ -f /proc/ppm/policy_status ]; then
            for idx in $(/vendor/bin/grep -E 'FORCE_LIMIT|PWR_THRO|THERMAL|USER_LIMIT' /proc/ppm/policy_status | /system/bin/awk -F'[][]' '{print $2}'); do
                zeshia "$idx 1" "/proc/ppm/policy_status"
            done

            for dx in $(/vendor/bin/grep -E 'SYS_BOOST' /proc/ppm/policy_status | /system/bin/awk -F'[][]' '{print $2}'); do
                zeshia "$dx 0" "/proc/ppm/policy_status"
            done
        fi
    fi

    # CPU POWER MODE
    zeshia "0" "/proc/cpufreq/cpufreq_cci_mode"
    zeshia "1" "/proc/cpufreq/cpufreq_power_mode"

    # GPU Frequency
    if [ -d /proc/gpufreq ]; then
        zeshia "0" /proc/gpufreq/gpufreq_opp_freq
    elif [ -d /proc/gpufreqv2 ]; then
        zeshia "-1" /proc/gpufreqv2/fix_target_opp_index
    fi

    # EAS/HMP Switch
    zeshia "1" /sys/devices/system/cpu/eas/enable

    # GPU Power limiter
    [ -f "/proc/gpufreq/gpufreq_power_limited" ] && {
        for setting in ignore_batt_oc ignore_batt_percent ignore_low_batt ignore_thermal_protect ignore_pbm_limited; do
            zeshia "$setting 0" /proc/gpufreq/gpufreq_power_limited
        done
    }

    # Batoc Throttling and Power Limiter>
    zeshia "0" /proc/perfmgr/syslimiter/syslimiter_force_disable
    zeshia "stop 0" /proc/mtk_batoc_throttling/battery_oc_protect_stop
    # Enable Power Budget management for new 5.x mtk kernels
    zeshia "stop 0" /proc/pbm/pbm_stop

    # Enable battery current limiter
    zeshia "stop 0" /proc/mtk_batoc_throttling/battery_oc_protect_stop

    # Eara Thermal
    zeshia "1" /sys/kernel/eara_thermal/enable

    # Restore UFS governor
    zeshia "-1" "/sys/devices/platform/10012000.dvfsrc/helio-dvfsrc/dvfsrc_req_ddr_opp"
    zeshia "-1" "/sys/kernel/helio-dvfsrc/dvfsrc_force_vcore_dvfs_opp"
    zeshia "userspace" "/sys/class/devfreq/mtk-dvfsrc-devfreq/governor"
    zeshia "userspace" "/sys/devices/platform/soc/1c00f000.dvfsrc/mtk-dvfsrc-devfreq/devfreq/mtk-dvfsrc-devfreq/governor"
}

###############################################
# # # # # # # SNAPDRAGON BALANCE # # # # # # #
###############################################
snapdragon_balance() {
    # Qualcomm CPU Bus and DRAM frequencies
    for path in /sys/class/devfreq/*cpu-ddr-latfloor*; do
        zeshia "compute" $path/governor
    done &

    for path in /sys/class/devfreq/*cpu*-lat; do
        zeshia "mem_latency" $path/governor
    done &

    for path in /sys/class/devfreq/*cpu-cpu-ddr-bw; do
        zeshia "bw_hwmon" $path/governor
    done &

    for path in /sys/class/devfreq/*cpu-cpu-llcc-bw; do
        zeshia "bw_hwmon" $path/governor
    done &

    if [ -d /sys/devices/system/cpu/bus_dcvs/LLCC ]; then
        max_freq=$(cat /sys/devices/system/cpu/bus_dcvs/LLCC/available_frequencies | tr ' ' '\n' | sort -nr | head -n 1)
        min_freq=$(cat /sys/devices/system/cpu/bus_dcvs/LLCC/available_frequencies | tr ' ' '\n' | sort -n | head -n 1)
        for path in /sys/devices/system/cpu/bus_dcvs/LLCC/*/max_freq; do
            zeshia $max_freq $path
        done &
        for path in /sys/devices/system/cpu/bus_dcvs/LLCC/*/min_freq; do
            zeshia $min_freq $path
        done &
    fi

    if [ -d /sys/devices/system/cpu/bus_dcvs/L3 ]; then
        max_freq=$(cat /sys/devices/system/cpu/bus_dcvs/L3/available_frequencies | tr ' ' '\n' | sort -nr | head -n 1)
        min_freq=$(cat /sys/devices/system/cpu/bus_dcvs/L3/available_frequencies | tr ' ' '\n' | sort -n | head -n 1)
        for path in /sys/devices/system/cpu/bus_dcvs/L3/*/max_freq; do
            zeshia $max_freq $path
        done &
        for path in /sys/devices/system/cpu/bus_dcvs/L3/*/min_freq; do
            zeshia $min_freq $path
        done &
    fi

    if [ -d /sys/devices/system/cpu/bus_dcvs/DDR ]; then
        max_freq=$(cat /sys/devices/system/cpu/bus_dcvs/DDR/available_frequencies | tr ' ' '\n' | sort -nr | head -n 1)
        min_freq=$(cat /sys/devices/system/cpu/bus_dcvs/DDR/available_frequencies | tr ' ' '\n' | sort -n | head -n 1)
        for path in /sys/devices/system/cpu/bus_dcvs/DDR/*/max_freq; do
            zeshia $max_freq $path
        done &
        for path in /sys/devices/system/cpu/bus_dcvs/DDR/*/min_freq; do
            zeshia $min_freq $path
        done &
    fi

    if [ -d /sys/devices/system/cpu/bus_dcvs/DDRQOS ]; then
        max_freq=$(cat /sys/devices/system/cpu/bus_dcvs/DDRQOS/available_frequencies | tr ' ' '\n' | sort -nr | head -n 1)
        min_freq=$(cat /sys/devices/system/cpu/bus_dcvs/DDRQOS/available_frequencies | tr ' ' '\n' | sort -n | head -n 1)
        for path in /sys/devices/system/cpu/bus_dcvs/DDRQOS/*/max_freq; do
            zeshia $max_freq $path
        done &
        for path in /sys/devices/system/cpu/bus_dcvs/DDRQOS/*/min_freq; do
            zeshia $min_freq $path
        done &
    fi

    # GPU Frequency
    gpu_path="/sys/class/kgsl/kgsl-3d0/devfreq"

    if [ -d $gpu_path ]; then
        max_freq=$(cat $gpu_path/available_frequencies | tr ' ' '\n' | sort -nr | head -n 1)
        min_freq=$(cat $gpu_path/available_frequencies | tr ' ' '\n' | sort -n | head -n 2)
        zeshia $min_freq $gpu_path/min_freq
        zeshia $max_freq $gpu_path/max_freq
    fi

    # GPU Bus
    for path in /sys/class/devfreq/*gpubw*; do
        zeshia "bw_vbif" $path/governor
    done &

    # Adreno Boost
    zeshia 1 /sys/class/kgsl/kgsl-3d0/devfreq/adrenoboost
}

###############################################
# # # # # # # EXYNOS BALANCE # # # # # # #
###############################################
exynos_balance() {
    # GPU Frequency
    gpu_path="/sys/kernel/gpu"
    [ -d "$gpu_path" ] && {
        max_freq=$(which_maxfreq "$gpu_path/gpu_available_frequencies")
        min_freq=$(which_minfreq "$gpu_path/gpu_available_frequencies")
        zeshia "$max_freq" "$gpu_path/gpu_max_clock"
        zeshia "$min_freq" "$gpu_path/gpu_min_clock"
    }

    mali_sysfs=$(find /sys/devices/platform/ -iname "*.mali" -print -quit 2>/dev/null)
    zeshia coarse_demand "$mali_sysfs/power_policy"

    # DRAM frequency
    [ $DEVICE_MITIGATION -eq 0 ] && {
        for path in /sys/class/devfreq/*devfreq_mif*; do
            devfreq_unlock "$path"
        done &
    }
}

###############################################
# # # # # # # UNISOC BALANCE # # # # # # #
###############################################
unisoc_balance() {
    # GPU Frequency
    gpu_path=$(find /sys/class/devfreq/ -type d -iname "*.gpu" -print -quit 2>/dev/null)
    [ -n "$gpu_path" ] && devfreq_unlock "$gpu_path"
}

###############################################
# # # # # # # TENSOR BALANCE # # # # # # #
###############################################
tensor_balance() {
    # GPU Frequency
    gpu_path=$(find /sys/devices/platform/ -type d -iname "*.mali" -print -quit 2>/dev/null)
    [ -n "$gpu_path" ] && {
        max_freq=$(which_maxfreq "$gpu_path/available_frequencies")
        min_freq=$(which_minfreq "$gpu_path/available_frequencies")
        zeshia "$max_freq" "$gpu_path/scaling_max_freq"
        zeshia "$min_freq" "$gpu_path/scaling_min_freq"
    }

    # DRAM frequency
    [ $DEVICE_MITIGATION -eq 0 ] && {
        for path in /sys/class/devfreq/*devfreq_mif*; do
            devfreq_unlock "$path"
        done &
    }
}

###############################################
# # # # # # #  BALANCED PROFILES! # # # # # # #
###############################################
balanced_profile() {
    AZLog "Applying Balanced Profile..."
    load_default_governor() {
        if [ -f "$DEFAULT_GOV_FILE" ]; then
            cat "$DEFAULT_GOV_FILE"
        else
            echo "schedutil"
        fi
    }

    # Load default cpu governor
    default_cpu_gov=$(load_default_governor)

    # Power level settings
    for pl in /sys/devices/system/cpu/perf; do
        zeshia 0 "$pl/gpu_pmu_enable"
        zeshia 0 "$pl/fuel_gauge_enable"
        zeshia 0 "$pl/enable"
        zeshia 1 "$pl/charger_enable"
    done

    # Restore CPU Scaling Governor
    setgov "$default_cpu_gov"

    # Restore Max CPU Frequency if its from ECO Mode or using Limit Frequency
    if [ -d /proc/ppm ]; then
        cluster=0
        for path in /sys/devices/system/cpu/cpufreq/policy*; do
            [ -f "$path/cpuinfo_max_freq" ] || continue

            cpu_maxfreq=$(cat "$path/cpuinfo_max_freq")
            cpu_minfreq=$(cat "$path/cpuinfo_min_freq")

            zeshia "$cluster $cpu_maxfreq" "/proc/ppm/policy/hard_userlimit_max_cpu_freq"
            zeshia "$cluster $cpu_minfreq" "/proc/ppm/policy/hard_userlimit_min_cpu_freq"

            ((cluster++))
        done
    fi

    for path in /sys/devices/system/cpu/*/cpufreq; do
        [ -f "$path/cpuinfo_max_freq" ] || continue

        cpu_maxfreq=$(cat "$path/cpuinfo_max_freq")
        cpu_minfreq=$(cat "$path/cpuinfo_min_freq")

        zeshia "$cpu_maxfreq" "$path/scaling_max_freq"
        zeshia "$cpu_minfreq" "$path/scaling_min_freq"
    done

    # vm cache pressure
    zeshia "120" "/proc/sys/vm/vfs_cache_pressure"

    # Workqueue settings
    zeshia "Y" /sys/module/workqueue/parameters/power_efficient
    zeshia "Y" /sys/module/workqueue/parameters/disable_numa
    zeshia "1" /sys/kernel/eara_thermal/enable
    zeshia "1" /sys/devices/system/cpu/eas/enable

    for path in /dev/stune/*; do
        base=$(basename "$path")
        if [[ "$base" == "top-app" || "$base" == "foreground" ]]; then
            zeshia 0 "$path/schedtune.boost"
            zeshia 0 "$path/schedtune.sched_boost_enabled"
        else
            zeshia 0 "$path/schedtune.boost"
            zeshia 0 "$path/schedtune.sched_boost_enabled"
        fi
        zeshia 0 "$path/schedtune.prefer_idle"
        zeshia 0 "$path/schedtune.colocate"
    done

    # Power level settings
    for pl in /sys/devices/system/cpu/perf; do
        zeshia 0 "$pl/gpu_pmu_enable"
        zeshia 0 "$pl/fuel_gauge_enable"
        zeshia 0 "$pl/enable"
        zeshia 1 "$pl/charger_enable"
    done

    # CPU Max Time Percent
    zeshia 100 /proc/sys/kernel/perf_cpu_time_max_percent

    zeshia 2 /proc/sys/kernel/perf_cpu_time_max_percent
    # Sched Energy Aware
    zeshia 1 /proc/sys/kernel/sched_energy_aware

    for cpucore in /sys/devices/system/cpu/cpu*; do
        zeshia 0 "$cpucore/core_ctl/enable"
        zeshia 0 "$cpucore/core_ctl/core_ctl_boost"
    done

    #  Disable battery saver module
    [ -f /sys/module/battery_saver/parameters/enabled ] && {
        if /vendor/bin/grep -qo '[0-9]\+' /sys/module/battery_saver/parameters/enabled; then
            zeshia 0 /sys/module/battery_saver/parameters/enabled
        else
            zeshia N /sys/module/battery_saver/parameters/enabled
        fi
    }

    #  Enable split lock mitigation
    zeshia 1 /proc/sys/kernel/split_lock_mitigate

    if [ -f "/sys/kernel/debug/sched_features" ]; then
        #  Consider scheduling tasks that are eager to run
        zeshia NEXT_BUDDY /sys/kernel/debug/sched_features
        #  Schedule tasks on their origin CPU if possible
        zeshia TTWU_QUEUE /sys/kernel/debug/sched_features
    fi

    case "$(cat /sdcard/config/soctype)" in
    1) mediatek_balance ;;
    2) snapdragon_balance ;;
    3) exynos_balance ;;
    4) unisoc_balance ;;
    5) tensor_balance ;;
    esac

}

###############################################
# # # # # # # MEDIATEK PERFORMANCE # # # # # # #
###############################################
mediatek_performance() {
    # PPM Settings
    if [ -d /proc/ppm ]; then
        if [ -f /proc/ppm/policy_status ]; then
            for idx in $(/vendor/bin/grep -E 'FORCE_LIMIT|PWR_THRO|THERMAL|USER_LIMIT' /proc/ppm/policy_status | /system/bin/awk -F'[][]' '{print $2}'); do
                zeshia "$idx 0" "/proc/ppm/policy_status"
            done

            for dx in $(/vendor/bin/grep -E 'SYS_BOOST' /proc/ppm/policy_status | /system/bin/awk -F'[][]' '{print $2}'); do
                zeshia "$dx 1" "/proc/ppm/policy_status"
            done
        fi
    fi

    # CPU Power Mode
    zeshia "1" "/proc/cpufreq/cpufreq_cci_mode"
    zeshia "3" "/proc/cpufreq/cpufreq_power_mode"

    # Max GPU Frequency
    if [ -d /proc/gpufreq ]; then
        gpu_freq="$(cat /proc/gpufreq/gpufreq_opp_dump | /vendor/bin/grep -o 'freq = [0-9]*' | sed 's/freq = //' | sort -nr | head -n 1)"
        zeshia "$gpu_freq" /proc/gpufreq/gpufreq_opp_freq
    elif [ -d /proc/gpufreqv2 ]; then
        zeshia 0 /proc/gpufreqv2/fix_target_opp_index
    fi

    # EAS/HMP Switch
    zeshia "0" /sys/devices/system/cpu/eas/enable

    # Disable GPU Power limiter
    [ -f "/proc/gpufreq/gpufreq_power_limited" ] && {
        for setting in ignore_batt_oc ignore_batt_percent ignore_low_batt ignore_thermal_protect ignore_pbm_limited; do
            zeshia "$setting 1" /proc/gpufreq/gpufreq_power_limited
        done
    }

    # Batoc battery and Power Limiter
    zeshia "0" /proc/perfmgr/syslimiter/syslimiter_force_disable
    zeshia "stop 1" /proc/mtk_batoc_throttling/battery_oc_protect_stop

    # Disable battery current limiter
    zeshia "stop 1" /proc/mtk_batoc_throttling/battery_oc_protect_stop

    # Eara Thermal
    zeshia "0" /sys/kernel/eara_thermal/enable

    # UFS Governor's
    zeshia "0" "/sys/devices/platform/10012000.dvfsrc/helio-dvfsrc/dvfsrc_req_ddr_opp"
    zeshia "0" "/sys/kernel/helio-dvfsrc/dvfsrc_force_vcore_dvfs_opp"
    zeshia "performance" "/sys/class/devfreq/mtk-dvfsrc-devfreq/governor"
    zeshia "performance" "/sys/devices/platform/soc/1c00f000.dvfsrc/mtk-dvfsrc-devfreq/devfreq/mtk-dvfsrc-devfreq/governor"

}

###############################################
# # # # # # # SNAPDRAGON PERFORMANCE # # # # # # #
###############################################
snapdragon_performance() {
    # Qualcomm CPU Bus and DRAM frequencies
    for path in /sys/class/devfreq/*cpu-ddr-latfloor*; do
        zeshia "performance" $path/governor
    done &

    for path in /sys/class/devfreq/*cpu*-lat; do
        zeshia "performance" $path/governor
    done &

    for path in /sys/class/devfreq/*cpu-cpu-ddr-bw; do
        zeshia "performance" $path/governor
    done &

    for path in /sys/class/devfreq/*cpu-cpu-llcc-bw; do
        zeshia "performance" $path/governor
    done &

    if [ -d /sys/devices/system/cpu/bus_dcvs/LLCC ]; then
        freq=$(cat /sys/devices/system/cpu/bus_dcvs/LLCC/available_frequencies | tr ' ' '\n' | sort -nr | head -n 1)
        for path in /sys/devices/system/cpu/bus_dcvs/LLCC/*/max_freq; do
            zeshia $freq $path
        done &
        for path in /sys/devices/system/cpu/bus_dcvs/LLCC/*/min_freq; do
            zeshia $freq $path
        done &
    fi

    if [ -d /sys/devices/system/cpu/bus_dcvs/L3 ]; then
        freq=$(cat /sys/devices/system/cpu/bus_dcvs/L3/available_frequencies | tr ' ' '\n' | sort -nr | head -n 1)
        for path in /sys/devices/system/cpu/bus_dcvs/L3/*/max_freq; do
            zeshia $freq $path
        done &
        for path in /sys/devices/system/cpu/bus_dcvs/L3/*/min_freq; do
            zeshia $freq $path
        done &
    fi

    if [ -d /sys/devices/system/cpu/bus_dcvs/DDR ]; then
        freq=$(cat /sys/devices/system/cpu/bus_dcvs/DDR/available_frequencies | tr ' ' '\n' | sort -nr | head -n 1)
        for path in /sys/devices/system/cpu/bus_dcvs/DDR/*/max_freq; do
            zeshia $freq $path
        done &
        for path in /sys/devices/system/cpu/bus_dcvs/DDR/*/min_freq; do
            zeshia $freq $path
        done &
    fi

    if [ -d /sys/devices/system/cpu/bus_dcvs/DDRQOS ]; then
        freq=$(cat /sys/devices/system/cpu/bus_dcvs/DDRQOS/available_frequencies | tr ' ' '\n' | sort -nr | head -n 1)
        for path in /sys/devices/system/cpu/bus_dcvs/DDRQOS/*/max_freq; do
            zeshia $freq $path
        done &
        for path in /sys/devices/system/cpu/bus_dcvs/DDRQOS/*/min_freq; do
            zeshia $freq $path
        done &
    fi

    # GPU Frequency
    gpu_path="/sys/class/kgsl/kgsl-3d0/devfreq"

    if [ -d $gpu_path ]; then
        freq=$(cat $gpu_path/available_frequencies | tr ' ' '\n' | sort -nr | head -n 1)
        zeshia $freq $gpu_path/min_freq
        zeshia $freq $gpu_path/max_freq
    fi

    # GPU Bus
    for path in /sys/class/devfreq/*gpubw*; do
        zeshia "performance" $path/governor
    done &

    # Adreno Boost
    zeshia 3 /sys/class/kgsl/kgsl-3d0/devfreq/adrenoboost
}

###############################################
# # # # # # # EXYNOS PERFORMANCE # # # # # # #
###############################################
exynos_performance() {
    # GPU Frequency
    gpu_path="/sys/kernel/gpu"
    [ -d "$gpu_path" ] && {
        max_freq=$(which_maxfreq "$gpu_path/gpu_available_frequencies")
        zeshia "$max_freq" "$gpu_path/gpu_max_clock"

        if [ $LITE_MODE -eq 1 ]; then
            mid_freq=$(which_midfreq "$gpu_path/gpu_available_frequencies")
            zeshia "$mid_freq" "$gpu_path/gpu_min_clock"
        else
            zeshia "$max_freq" "$gpu_path/gpu_min_clock"
        fi
    }

    mali_sysfs=$(find /sys/devices/platform/ -iname "*.mali" -print -quit 2>/dev/null)
    zeshia always_on "$mali_sysfs/power_policy"

    # DRAM and Buses Frequency
    [ $DEVICE_MITIGATION -eq 0 ] && {
        for path in /sys/class/devfreq/*devfreq_mif*; do
            [ $LITE_MODE -eq 1 ] &&
                devfreq_mid_perf "$path" ||
                devfreq_max_perf "$path"
        done &
    }
}

###############################################
# # # # # # # UNISOC PERFORMANCE # # # # # # #
###############################################
unisoc_performance() {
    # GPU Frequency
    gpu_path=$(find /sys/class/devfreq/ -type d -iname "*.gpu" -print -quit 2>/dev/null)
    [ -n "$gpu_path" ] && {
        if [ $LITE_MODE -eq 0 ]; then
            devfreq_max_perf "$gpu_path"
        else
            devfreq_mid_perf "$gpu_path"
        fi
    }
}

###############################################
# # # # # # # TENSOR PERFORMANCE # # # # # # #
###############################################
tensor_performance() {
    # GPU Frequency
    gpu_path=$(find /sys/devices/platform/ -type d -iname "*.mali" -print -quit 2>/dev/null)
    [ -n "$gpu_path" ] && {
        max_freq=$(which_maxfreq "$gpu_path/available_frequencies")
        zeshia "$max_freq" "$gpu_path/scaling_max_freq"

        if [ $LITE_MODE -eq 1 ]; then
            mid_freq=$(which_midfreq "$gpu_path/available_frequencies")
            zeshia "$mid_freq" "$gpu_path/scaling_min_freq"
        else
            zeshia "$max_freq" "$gpu_path/scaling_min_freq"
        fi
    }

    # DRAM frequency
    [ $DEVICE_MITIGATION -eq 0 ] && {
        for path in /sys/class/devfreq/*devfreq_mif*; do
            [ $LITE_MODE -eq 1 ] &&
                devfreq_mid_perf "$path" ||
                devfreq_max_perf "$path"
        done &
    }
}

###############################################
# # # # # # # PERFORMANCE PROFILE! # # # # # # #
###############################################

performance_profile() {
    AZLog "Applying Performance Profile..."
    load_game_governor() {
        if [ -f "$GAME_GOV_FILE" ]; then
            cat "$GAME_GOV_FILE"
        else
            echo "schedutil"
        fi
    }
    
    # Save governor
    CPU="/sys/devices/system/cpu/cpu0/cpufreq"
    chmod 644 "$CPU/scaling_governor"
    default_gov=$(cat "$CPU/scaling_governor")
    echo "$default_gov" >$DEFAULT_GOV_FILE

    # Load default cpu governor
    game_cpu_gov=$(load_game_governor)

    # Power level settings
    for pl in /sys/devices/system/cpu/perf; do
        zeshia 1 "$pl/gpu_pmu_enable"
        zeshia 1 "$pl/fuel_gauge_enable"
        zeshia 1 "$pl/enable"
        zeshia 1 "$pl/charger_enable"
    done

    # Set Governor Game
    setgov "performance"

        if [ -d /proc/ppm ]; then
            cluster=0
            for path in /sys/devices/system/cpu/cpufreq/policy*; do
                cpu_maxfreq=$(cat "$path/cpuinfo_max_freq")

                zeshia "$cluster $cpu_maxfreq" "/proc/ppm/policy/hard_userlimit_max_cpu_freq"
                zeshia "$cluster $cpu_maxfreq" "/proc/ppm/policy/hard_userlimit_min_cpu_freq"
                ((cluster++))

            done
        fi
        for path in /sys/devices/system/cpu/*/cpufreq; do
            cpu_maxfreq=$(cat "$path/cpuinfo_max_freq")

            zeshia "$cpu_maxfreq" "$path/scaling_max_freq"
            zeshia "$cpu_maxfreq" "$path/scaling_min_freq"

        done

    # VM Cache Pressure
    zeshia "40" "/proc/sys/vm/vfs_cache_pressure"
    zeshia "3" "/proc/sys/vm/drop_caches"

    # Workqueue settings
    zeshia "N" /sys/module/workqueue/parameters/power_efficient
    zeshia "N" /sys/module/workqueue/parameters/disable_numa
    zeshia "0" /sys/kernel/eara_thermal/enable
    zeshia "0" /sys/devices/system/cpu/eas/enable
    zeshia "1" /sys/devices/system/cpu/cpu2/online
    zeshia "1" /sys/devices/system/cpu/cpu3/online

    # Schedtune Settings
    for path in /dev/stune/*; do
        base=$(basename "$path")
        if [[ "$base" == "top-app" || "$base" == "foreground" ]]; then
            zeshia 30 "$path/schedtune.boost"
            zeshia 1 "$path/schedtune.sched_boost_enabled"
        else
            zeshia 30 "$path/schedtune.boost"
            zeshia 1 "$path/schedtune.sched_boost_enabled"
        fi
        zeshia 0 "$path/schedtune.prefer_idle"
        zeshia 0 "$path/schedtune.colocate"
    done

    # Power level settings
    for pl in /sys/devices/system/cpu/perf; do
        zeshia 1 "$pl/gpu_pmu_enable"
        zeshia 1 "$pl/fuel_gauge_enable"
        zeshia 1 "$pl/enable"
        zeshia 1 "$pl/charger_enable"
    done

    # CPU max tune percent
    zeshia 1 /proc/sys/kernel/perf_cpu_time_max_percent

    # Sched Energy Aware
    zeshia 1 /proc/sys/kernel/sched_energy_aware

    # CPU Core control Boost
    for cpucore in /sys/devices/system/cpu/cpu*; do
        zeshia 0 "$cpucore/core_ctl/enable"
        zeshia 0 "$cpucore/core_ctl/core_ctl_boost"
    done

    clear_background_apps() {
        AZLog "Clearing background apps..."

        # Get the list of running apps sorted by CPU usage (excluding system processes and the script itself)
        app_list=$(top -n 1 -o %CPU | /system/bin/awk 'NR>7 {print $1}' | while read -r pid; do
            pkg=$(cmd package list packages -U | /system/bin/awk -v pid="$pid" '$2 == pid {print $1}' | cut -d':' -f2)
            if [ -n "$pkg" ] && ! echo "$pkg" | /vendor/bin/grep -qE "com.android.systemui|com.android.settings|$(basename "$0")"; then
                echo "$pkg"
            fi
        done)

        # Kill apps in order of highest CPU usage
        for app in $app_list; do
            am force-stop "$app"
            AZLog "Stopped app: $app"
        done

        # force stop
        am force-stop com.instagram.android
        am force-stop com.android.vending
        am force-stop app.grapheneos.camera
        am force-stop com.google.android.gm
        am force-stop com.google.android.apps.youtube.creator
        am force-stop com.dolby.ds1appUI
        am force-stop com.google.android.youtube
        am force-stop com.twitter.android
        am force-stop nekox.messenger
        am force-stop com.shopee.id
        am force-stop com.vanced.android.youtube
        am force-stop com.speedsoftware.rootexplorer
        am force-stop com.bukalapak.android
        am force-stop org.telegram.messenger
        am force-stop ru.zdevs.zarchiver
        am force-stop com.android.chrome
        am force-stop com.whatsapp.messenger
        am force-stop com.google.android.GoogleCameraEng
        am force-stop com.facebook.orca
        am force-stop com.lazada.android
        am force-stop com.android.camera
        am force-stop com.android.settings
        am force-stop com.franco.kernel
        am force-stop com.telkomsel.telkomselcm
        am force-stop com.facebook.katana
        am force-stop com.instagram.android
        am force-stop com.facebook.lite
        am kill-all
    }
    if [ "$(cat /data/adb/.config/AZenith/clearbg)" -eq 1 ]; then
        clear_background_apps
        AZLog "Clearing apps"
    fi

    # Disable battery saver module
    [ -f /sys/module/battery_saver/parameters/enabled ] && {
        if /vendor/bin/grep -qo '[0-9]\+' /sys/module/battery_saver/parameters/enabled; then
            zeshia 0 /sys/module/battery_saver/parameters/enabled
        else
            zeshia N /sys/module/battery_saver/parameters/enabled
        fi
    }

    # Disable split lock mitigation
    zeshia 0 /proc/sys/kernel/split_lock_mitigate

    # Schedfeatures settings
    if [ -f "/sys/kernel/debug/sched_features" ]; then
        zeshia NEXT_BUDDY /sys/kernel/debug/sched_features
        zeshia NO_TTWU_QUEUE /sys/kernel/debug/sched_features
    fi

    if [ "$(cat /data/adb/.config/AZenith/bypass_charge)" -eq 1 ]; then
        bypassCharge 1
    fi

    case "$(cat /sdcard/config/soctype)" in
    1) mediatek_performance ;;
    2) snapdragon_performance ;;
    3) exynos_performance ;;
    4) unisoc_performance ;;
    5) tensor_performance ;;
    esac

}
###############################################
# # # # # # # MEDIATEK POWERSAVE # # # # # # #
###############################################
mediatek_powersave() {
    # PPM Settings
    if [ -d /proc/ppm ]; then
        if [ -f /proc/ppm/policy_status ]; then
            for idx in $(/vendor/bin/grep -E 'FORCE_LIMIT|PWR_THRO|THERMAL|USER_LIMIT' /proc/ppm/policy_status | /system/bin/awk -F'[][]' '{print $2}'); do
                zeshia "$idx 1" "/proc/ppm/policy_status"
            done

            for dx in $(/vendor/bin/grep -E 'SYS_BOOST' /proc/ppm/policy_status | /system/bin/awk -F'[][]' '{print $2}'); do
                zeshia "$dx 0" "/proc/ppm/policy_status"
            done
        fi
    fi

    # UFS governor
    zeshia "0" "/sys/devices/platform/10012000.dvfsrc/helio-dvfsrc/dvfsrc_req_ddr_opp"
    zeshia "0" "/sys/kernel/helio-dvfsrc/dvfsrc_force_vcore_dvfs_opp"
    zeshia "powersave" "/sys/class/devfreq/mtk-dvfsrc-devfreq/governor"
    zeshia "powersave" "/sys/devices/platform/soc/1c00f000.dvfsrc/mtk-dvfsrc-devfreq/devfreq/mtk-dvfsrc-devfreq/governor"

    # GPU Power limiter - Performance mode (not for Powersave)
    [ -f "/proc/gpufreq/gpufreq_power_limited" ] && {
        for setting in ignore_batt_oc ignore_batt_percent ignore_low_batt ignore_thermal_protect ignore_pbm_limited; do
            zeshia "$setting 1" /proc/gpufreq/gpufreq_power_limited
        done

    }

    # Batoc Throttling and Power Limiter>
    zeshia "0" /proc/perfmgr/syslimiter/syslimiter_force_disable
    zeshia "stop 0" /proc/mtk_batoc_throttling/battery_oc_protect_stop
    # Enable Power Budget management for new 5.x mtk kernels
    zeshia "stop 0" /proc/pbm/pbm_stop

    # Enable battery current limiter
    zeshia "stop 0" /proc/mtk_batoc_throttling/battery_oc_protect_stop

    # Eara Thermal
    zeshia "1" /sys/kernel/eara_thermal/enable

}
###############################################
# # # # # # # SNAPDRAGON POWERSAVE # # # # # # #
###############################################
snapdragon_powersave() {
    # Qualcomm CPU Bus and DRAM frequencies
    for path in /sys/class/devfreq/*cpu-ddr-latfloor*; do
        zeshia "powersave" $path/governor
    done &

    for path in /sys/class/devfreq/*cpu*-lat; do
        zeshia "powersave" $path/governor
    done &

    for path in /sys/class/devfreq/*cpu-cpu-ddr-bw; do
        zeshia "powersave" $path/governor
    done &

    for path in /sys/class/devfreq/*cpu-cpu-llcc-bw; do
        zeshia "powersave" $path/governor
    done &

    if [ -d /sys/devices/system/cpu/bus_dcvs/LLCC ]; then
        freq=$(cat /sys/devices/system/cpu/bus_dcvs/LLCC/available_frequencies | tr ' ' '\n' | sort -n | head -n 1)
        for path in /sys/devices/system/cpu/bus_dcvs/LLCC/*/max_freq; do
            zeshia $freq $path
        done &
        for path in /sys/devices/system/cpu/bus_dcvs/LLCC/*/min_freq; do
            zeshia $freq $path
        done &
    fi

    if [ -d /sys/devices/system/cpu/bus_dcvs/L3 ]; then
        freq=$(cat /sys/devices/system/cpu/bus_dcvs/L3/available_frequencies | tr ' ' '\n' | sort -n | head -n 1)
        for path in /sys/devices/system/cpu/bus_dcvs/L3/*/max_freq; do
            zeshia $freq $path
        done &
        for path in /sys/devices/system/cpu/bus_dcvs/L3/*/min_freq; do
            zeshia $freq $path
        done &
    fi

    if [ -d /sys/devices/system/cpu/bus_dcvs/DDR ]; then
        freq=$(cat /sys/devices/system/cpu/bus_dcvs/DDR/available_frequencies | tr ' ' '\n' | sort -n | head -n 1)
        for path in /sys/devices/system/cpu/bus_dcvs/DDR/*/max_freq; do
            zeshia $freq $path
        done &
        for path in /sys/devices/system/cpu/bus_dcvs/DDR/*/min_freq; do
            zeshia $freq $path
        done &
    fi

    if [ -d /sys/devices/system/cpu/bus_dcvs/DDRQOS ]; then
        freq=$(cat /sys/devices/system/cpu/bus_dcvs/DDRQOS/available_frequencies | tr ' ' '\n' | sort -n | head -n 1)
        for path in /sys/devices/system/cpu/bus_dcvs/DDRQOS/*/max_freq; do
            zeshia $freq $path
        done &
        for path in /sys/devices/system/cpu/bus_dcvs/DDRQOS/*/min_freq; do
            zeshia $freq $path
        done &
    fi

    # GPU Frequency
    gpu_path="/sys/class/kgsl/kgsl-3d0/devfreq"

    if [ -d $gpu_path ]; then
        freq=$(cat $gpu_path/available_frequencies | tr ' ' '\n' | sort -n | head -n 2)
        zeshia $freq $gpu_path/min_freq
        zeshia $freq $gpu_path/max_freq
    fi

    # GPU Bus
    for path in /sys/class/devfreq/*gpubw*; do
        zeshia "powersave" $path/governor
    done &

    # Adreno Boost
    zeshia 0 /sys/class/kgsl/kgsl-3d0/devfreq/adrenoboost
}

###############################################
# # # # # # # EXYNOS POWERSAVE # # # # # # #
###############################################
exynos_powersave() {
    # GPU Frequency
    gpu_path="/sys/kernel/gpu"
    [ -d "$gpu_path" ] && {
        freq=$(which_minfreq "$gpu_path/gpu_available_frequencies")
        zeshia "$freq" "$gpu_path/gpu_min_clock"
        zeshia "$freq" "$gpu_path/gpu_max_clock"
    }
}

###############################################
# # # # # # # UNISOC POWERSAVE # # # # # # #
###############################################
unisoc_powersave() {
    # GPU Frequency
    gpu_path=$(find /sys/class/devfreq/ -type d -iname "*.gpu" -print -quit 2>/dev/null)
    [ -n "$gpu_path" ] && devfreq_min_perf "$gpu_path"
}

###############################################
# # # # # # # TENSOR POWERSAVE # # # # # # #
###############################################
tensor_powersave() {
    # GPU Frequency
    gpu_path=$(find /sys/devices/platform/ -type d -iname "*.mali" -print -quit 2>/dev/null)
    [ -n "$gpu_path" ] && {
        freq=$(which_minfreq "$gpu_path/available_frequencies")
        zeshia "$freq" "$gpu_path/scaling_min_freq"
        zeshia "$freq" "$gpu_path/scaling_max_freq"
    }
}

###############################################
# # # # # # # POWERSAVE PROFILE # # # # # # #
###############################################

eco_mode() {
    AZLog "Applying Eco (Powersave) Profile..."
    # Load Powersave Governor
    load_powersave_governor() {
        if [ -f "$POWERSAVE_GOV_FILE" ]; then
            cat "$POWERSAVE_GOV_FILE"
        else
            echo "powersave"
        fi
    }
    powersave_cpu_gov=$(load_powersave_governor)

    setgov "powersave"

    # Power level settings
    for pl in /sys/devices/system/cpu/perf; do
        zeshia 0 "$pl/gpu_pmu_enable"
        zeshia 0 "$pl/fuel_gauge_enable"
        zeshia 0 "$pl/enable"
        zeshia 1 "$pl/charger_enable"
    done

    # Disable DND
    if [ "$(cat /data/adb/.config/AZenith/dnd)" -eq 1 ]; then
        cmd notification set_dnd off && AZLog "DND disabled" || AZLog "Failed to disable DND"
    fi

    # Limit cpu freq
    if [ -d /proc/ppm ]; then
        cluster=0
        for path in /sys/devices/system/cpu/cpufreq/policy*; do
            cpu_maxfreq=$(cat "$path/cpuinfo_max_freq")
            cpu_minfreq=$(cat "$path/cpuinfo_min_freq")

            zeshia "$cluster $cpu_maxfreq" "/proc/ppm/policy/hard_userlimit_max_cpu_freq"
            zeshia "$cluster $cpu_minfreq" "/proc/ppm/policy/hard_userlimit_min_cpu_freq"
            ((cluster++))
        done
    fi
    for path in /sys/devices/system/cpu/cpufreq/policy*; do
        cpu_maxfreq=$(cat "$path/cpuinfo_max_freq")
        cpu_minfreq=$(cat "$path/cpuinfo_min_freq")

        zeshia "$cpu_maxfreq" "$path/scaling_max_freq"
        zeshia "$cpu_minfreq" "$path/scaling_min_freq"
    done

    # VM Cache Pressure
    zeshia "120" "/proc/sys/vm/vfs_cache_pressure"

    zeshia 0 /proc/sys/kernel/perf_cpu_time_max_percent
    zeshia 0 /proc/sys/kernel/sched_energy_aware

    #  Enable battery saver module
    [ -f /sys/module/battery_saver/parameters/enabled ] && {
        if /vendor/bin/grep -qo '[0-9]\+' /sys/module/battery_saver/parameters/enabled; then
            zeshia 1 /sys/module/battery_saver/parameters/enabled
        else
            zeshia Y /sys/module/battery_saver/parameters/enabled
        fi
    }

    # Schedfeature settings
    if [ -f "/sys/kernel/debug/sched_features" ]; then
        zeshia NO_NEXT_BUDDY /sys/kernel/debug/sched_features
        zeshia NO_TTWU_QUEUE /sys/kernel/debug/sched_features
    fi

    case "$(cat /sdcard/config/soctype)" in
    1) mediatek_powersave ;;
    2) snapdragon_powersave ;;
    3) exynos_powersave ;;
    4) unisoc_powersave ;;
    5) tensor_powersave ;;
    esac

}

###############################################
# # # # # # # INITIALIZE # # # # # # #
###############################################

initialize() {
    AZLog "Initializing AZenith..."
    # Disable all kernel panic mechanisms
    for param in hung_task_timeout_secs panic_on_oom panic_on_oops panic softlockup_panic; do
        zeshia "0" "/proc/sys/kernel/$param"
    done

    # Tweaking scheduler to reduce latency
    zeshia 500000 /proc/sys/kernel/sched_migration_cost_ns
    zeshia 1000000 /proc/sys/kernel/sched_min_granularity_ns
    zeshia 500000 /proc/sys/kernel/sched_wakeup_granularity_ns
    # Disable read-ahead for swap devices
    zeshia 0 /proc/sys/vm/page-cluster
    # Update /proc/stat less often to reduce jitter
    zeshia 20 /proc/sys/vm/stat_interval
    # Disable compaction_proactiveness
    zeshia 0 /proc/sys/vm/compaction_proactiveness

    zeshia 255 /proc/sys/kernel/sched_lib_mask_force

    CPU="/sys/devices/system/cpu/cpu0/cpufreq"
    chmod 666 "$CPU/scaling_governor"
    default_gov=$(cat "$CPU/scaling_governor")
    echo "$default_gov" > "$DEFAULT_GOV_FILE"

# Apply Tweaks Based on Chipset
chipset=$(/vendor/bin/grep -i 'hardware' /proc/cpuinfo | uniq | cut -d ':' -f2 | sed 's/^[ \t]*//')
[ -z "$chipset" ] && chipset="$(getprop ro.board.platform) $(getprop ro.hardware)"

case "$(echo "$chipset" | tr '[:upper:]' '[:lower:]')" in
*mt* | *MT*)
    soc="MediaTek"
    echo 1 > "/sdcard/config/soctype"
    ;;
*sm* | *qcom* | *SM* | *QCOM* | *Qualcomm* | *sdm* | *snapdragon*)
    soc="Snapdragon"
    echo 2 > "/sdcard/config/soctype"
    ;;
*exynos* | *Exynos* | *EXYNOS* | *universal* | *samsung* | *erd* | *s5e*)
    soc="Exynos"
    echo 3 > "/sdcard/config/soctype"
    ;;
*Unisoc* | *unisoc* | *ums*)
    soc="Unisoc"
    echo 4 > "/sdcard/config/soctype"
    ;;
*gs* | *Tensor* | *tensor*)
    soc="Tensor"
    echo 5 > "/sdcard/config/soctype"
    ;;
*)
    soc="Unknown"
    echo 0 > "/sdcard/config/soctype"
    ;;
esac

    sync
}

###############################################

###############################################
# # # # # # # MAIN FUNCTION! # # # # # # #
###############################################
AZLog "AZenith script started with argument: $1"
case "$1" in
0) initialize ;;
1) performance_profile ;;
2) balanced_profile ;;
3) eco_mode ;;
*) AZLog "Invalid argument: $1" ;;
esac
$@
wait
AZLog "AZenith script finished."
exit
