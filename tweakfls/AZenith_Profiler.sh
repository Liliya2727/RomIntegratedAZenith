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
AZLog() {
	if [ "$(getprop persist.sys.azenith-debug)" = "true" ]; then
		local message log_tag
		message="$1"
		log_tag="AZenith"
		log -t "$log_tag" "$message"
	fi
}
dlog() {
	local message log_tag
	message="$1"
	log_tag="AZenith"
	log -t "$log_tag" "$message"
}

# fix dumpsys
export PATH="/product/bin:/apex/com.android.runtime/bin:/apex/com.android.art/bin:/system_ext/bin:/system/bin:/system/xbin:/odm/bin:/vendor/bin:/vendor/xbin"
AZLog "Runtime PATH was set to: $PATH"

zeshia() {
	local value="$1"
	local path="$2"
	local pathname
	pathname="$(/system/bin/echo "$path" | awk -F'/' '{print $(NF-1)"/"$NF}')"
	if [ ! -e "$path" ]; then
		AZLog "File /$pathname not found, skipping..."
		return
	fi
	if [ ! -w "$path" ] && ! chmod 644 "$path" 2>/dev/null; then
		AZLog "Cannot write to /$pathname (permission denied)"
		return
	fi
	/system/bin/echo "$value" >"$path" 2>/dev/null
	local current
	current="$(cat "$path" 2>/dev/null)"
	if [ "$current" = "$value" ]; then
		AZLog "Set /$pathname to $value"
	else
		/system/bin/echo "$value" >"$path" 2>/dev/null
		current="$(cat "$path" 2>/dev/null)"
		if [ "$current" = "$value" ]; then
			AZLog "Set $pathname to $value (after retry)"
		else
			AZLog "Failed to set /$pathname to $value"
		fi
	fi
	chmod 444 "$path" 2>/dev/null
}

zeshiax() {
	local value="$1"
	local path="$2"
	local pathname
	pathname="$(/system/bin/echo "$path" | awk -F'/' '{print $(NF-1)"/"$NF}')"
	if [ ! -e "$path" ]; then
		AZLog "File /$pathname not found, skipping..."
		return
	fi
	if [ ! -w "$path" ] && ! chmod 644 "$path" 2>/dev/null; then
		AZLog "Cannot write to /$pathname (permission denied)"
		return
	fi
	/system/bin/echo "$value" >"$path" 2>/dev/null
	local current
	current="$(cat "$path" 2>/dev/null)"
	if [ "$current" = "$value" ]; then
		AZLog "Set /$pathname to $value"
	else
		/system/bin/echo "$value" >"$path" 2>/dev/null
		current="$(cat "$path" 2>/dev/null)"
		if [ "$current" = "$value" ]; then
			AZLog "Set $pathname to $value (after retry)"
		else
			AZLog "Failed to set /$pathname to $value"
		fi
	fi
}

setfreq() {
	local file="$1" target="$2" chosen=""
	if [ -f "$file" ]; then
		chosen=$(tr -s ' ' '\n' <"$file" |
			awk -v t="$target" '
                {diff = (t - $1 >= 0 ? t - $1 : $1 - t)}
                NR==1 || diff < mindiff {mindiff = diff; val=$1}
                END {print val}')
	else
		chosen="$target"
	fi
	/system/bin/echo "$chosen"
}

setgov() {
	AZLog "Setting CPU governor to '$1'"
	chmod 644 /sys/devices/system/cpu/cpu*/cpufreq/scaling_governor
	/system/bin/echo "$1" | tee /sys/devices/system/cpu/cpu*/cpufreq/scaling_governor >/dev/null
	chmod 444 /sys/devices/system/cpu/cpu*/cpufreq/scaling_governor
	chmod 444 /sys/devices/system/cpu/cpufreq/policy*/scaling_governor
}

setsfreqs() {
	limiter=$(getprop persist.sys.azenithconf.freqoffset | sed -e 's/Disabled/100/' -e 's/%//g')
	curprofile=$(getprop sys.azenith.currentprofile 2>/dev/null)
	if [ -d /proc/ppm ]; then
		cluster=0
		for path in /sys/devices/system/cpu/cpufreq/policy*; do
			cpu_maxfreq=$(cat "$path/cpuinfo_max_freq")
			cpu_minfreq=$(cat "$path/cpuinfo_min_freq")
			new_max_target=$((cpu_maxfreq * limiter / 100))
			new_maxfreq=$(setfreq "$path/scaling_available_frequencies" "$new_max_target")
			[ "$curprofile" = "3" ] && {
				target_min_target=$((cpu_maxfreq * 40 / 100))
				new_minfreq=$(setfreq "$path/scaling_available_frequencies" "$target_min_target")
				zeshia "$cluster $new_maxfreq" "/proc/ppm/policy/hard_userlimit_max_cpu_freq"
				zeshia "$cluster $new_minfreq" "/proc/ppm/policy/hard_userlimit_min_cpu_freq"
				((cluster++))
				continue
			}
			zeshia "$cluster $new_maxfreq" "/proc/ppm/policy/hard_userlimit_max_cpu_freq"
			zeshia "$cluster $cpu_minfreq" "/proc/ppm/policy/hard_userlimit_min_cpu_freq"
			((cluster++))
		done
	fi
	for path in /sys/devices/system/cpu/*/cpufreq; do
		cpu_maxfreq=$(cat "$path/cpuinfo_max_freq")
		cpu_minfreq=$(cat "$path/cpuinfo_min_freq")
		new_max_target=$((cpu_maxfreq * limiter / 100))
		new_maxfreq=$(setfreq "$path/scaling_available_frequencies" "$new_max_target")
		[ "$curprofile" = "3" ] && {
			target_min_target=$((cpu_maxfreq * 40 / 100))
			new_minfreq=$(setfreq "$path/scaling_available_frequencies" "$target_min_target")
			zeshia "$new_maxfreq" "$path/scaling_max_freq"
			zeshia "$new_minfreq" "$path/scaling_min_freq"
			continue
		}
		zeshia "$new_maxfreq" "$path/scaling_max_freq"
		zeshia "$cpu_minfreq" "$path/scaling_min_freq"
		chmod -f 444 /sys/devices/system/cpu/cpufreq/policy*/scaling_*_freq
	done
}

apply_game_freqs() {
	# Fix Target OPP Index
	if [ -d /proc/ppm ]; then
		cluster=-1
		for path in /sys/devices/system/cpu/cpufreq/policy*; do
			((cluster++))
			cpu_maxfreq=$(cat "$path/cpuinfo_max_freq")
			cpu_minfreq=$(cat "$path/cpuinfo_max_freq")
			[ "$(getprop persist.sys.azenithconf.cpulimit)" -eq 1 ] && {
				new_maxtarget=$((cpu_maxfreq * 80 / 100))
				new_midtarget=$((cpu_maxfreq * 40 / 100))
				new_midfreq=$(setfreq "$path/scaling_available_frequencies" "$new_midtarget")
				new_maxfreq=$(setfreq "$path/scaling_available_frequencies" "$new_maxtarget")
				zeshia "$cluster $new_maxfreq" "/proc/ppm/policy/hard_userlimit_max_cpu_freq"
				zeshia "$cluster $new_midfreq" "/proc/ppm/policy/hard_userlimit_min_cpu_freq"
				continue
			}
			zeshia "$cluster $cpu_maxfreq" "/proc/ppm/policy/hard_userlimit_max_cpu_freq"
			zeshia "$cluster $cpu_minfreq" "/proc/ppm/policy/hard_userlimit_min_cpu_freq"
		done
	fi
	for path in /sys/devices/system/cpu/*/cpufreq; do
		cpu_maxfreq=$(cat "$path/cpuinfo_max_freq")
		cpu_minfreq=$(cat "$path/cpuinfo_max_freq")
		[ "$(getprop persist.sys.azenithconf.cpulimit)" -eq 1 ] && {
			new_maxtarget=$((cpu_maxfreq * 80 / 100))
			new_midtarget=$((cpu_maxfreq * 40 / 100))
			new_midfreq=$(setfreq "$path/scaling_available_frequencies" "$new_midtarget")
			new_maxfreq=$(setfreq "$path/scaling_available_frequencies" "$new_maxtarget")
			zeshia "$new_maxfreq" "$path/scaling_max_freq"
			zeshia "$new_midfreq" "$path/scaling_min_freq"
			continue
		}
		zeshia "$cpu_maxfreq" "$path/scaling_max_freq"
		zeshia "$cpu_minfreq" "$path/scaling_min_freq"
		chmod -f 644 /sys/devices/system/cpu/cpufreq/policy*/scaling_*_freq
	done
}

sync

###############################################
# # # # # # #  BALANCED PROFILES! # # # # # # #
###############################################
balanced_profile() {

	# Load default cpu governor
	default_cpu_gov=$(getprop persist.sys.azenith.defaultgov)

	# Power level settings
	for pl in /sys/devices/system/cpu/perf; do
		zeshia 0 "$pl/gpu_pmu_enable"
		zeshia 0 "$pl/fuel_gauge_enable"
		zeshia 0 "$pl/enable"
		zeshia 1 "$pl/charger_enable"
	done

	# Restore CPU Scaling Governor
	setgov "$default_cpu_gov" && dlog "Restoring governor to : $default_cpu_gov"

	# Restore Max CPU Frequency if its from ECO Mode or using Limit Frequency
	setsfreqs

	# vm cache pressure
	zeshia "120" "/proc/sys/vm/vfs_cache_pressure"

	# Skip If Lite Mode Enabled
	if [ "$(getprop persist.sys.azenithconf.cpulimit)" -eq 0 ]; then
		# Workqueue settings
		zeshia "Y" /sys/module/workqueue/parameters/power_efficient
		zeshia "Y" /sys/module/workqueue/parameters/disable_numa
		zeshia "1" /sys/kernel/eara_thermal/enable
		zeshia "1" /sys/devices/system/cpu/eas/enable
	fi

	# Power level settings
	for pl in /sys/devices/system/cpu/perf; do
		zeshia 0 "$pl/gpu_pmu_enable"
		zeshia 0 "$pl/fuel_gauge_enable"
		zeshia 0 "$pl/enable"
		zeshia 1 "$pl/charger_enable"
	done

	if [ "$(/system/bin/getprop persist.sys.azenithconf.dndongaming)" -eq 1 ]; then
		cmd notification set_dnd off && AZLog "DND disabled"
	fi

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

	# Skip If Lite Mode Enabled
	if [ "$(getprop persist.sys.azenithconf.cpulimit)" -eq 0 ]; then
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
	fi

	# CPU POWER MODE
	zeshia "0" "/proc/cpufreq/cpufreq_cci_mode"
	zeshia "1" "/proc/cpufreq/cpufreq_power_mode"

	# Skip If Lite Mode Enabled
	if [ "$(getprop persist.sys.azenithconf.cpulimit)" -eq 0 ]; then
		# GPU Frequency
		if [ -d /proc/gpufreq ]; then
			zeshia "0" /proc/gpufreq/gpufreq_opp_freq
		elif [ -d /proc/gpufreqv2 ]; then
			zeshia "-1" /proc/gpufreqv2/fix_target_opp_index
		fi
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

	# Skip If Lite Mode Enabled
	if [ "$(getprop persist.sys.azenithconf.cpulimit)" -eq 0 ]; then
		# Restore UFS governor
		zeshia "-1" "/sys/devices/platform/10012000.dvfsrc/helio-dvfsrc/dvfsrc_req_ddr_opp"
		zeshia "-1" "/sys/kernel/helio-dvfsrc/dvfsrc_force_vcore_dvfs_opp"
		zeshia "userspace" "/sys/class/devfreq/mtk-dvfsrc-devfreq/governor"
		zeshia "userspace" "/sys/devices/platform/soc/1c00f000.dvfsrc/mtk-dvfsrc-devfreq/devfreq/mtk-dvfsrc-devfreq/governor"
	fi
}

###############################################
# # # # # # # PERFORMANCE PROFILE! # # # # # # #
###############################################

performance_profile() {
	AZLog "Applying Performance Profile..."

	# Save governor
	CPU="/sys/devices/system/cpu/cpu0/cpufreq"
	chmod 644 "$CPU/scaling_governor"
	default_gov=$(cat "$CPU/scaling_governor")
	setprop persist.sys.azenith.defaultgov "$default_gov"

	# Power level settings
	for pl in /sys/devices/system/cpu/perf; do
		zeshia 1 "$pl/gpu_pmu_enable"
		zeshia 1 "$pl/fuel_gauge_enable"
		zeshia 1 "$pl/enable"
		zeshia 1 "$pl/charger_enable"
	done

	# Apply Game Governor
	[ "$(getprop persist.sys.azenithconf.cpulimit)" -eq 0 ] &&
		setgov "performance" && dlog "Applying governor to : performance" ||
		setgov "$default_gov" && dlog "Applying governor to : $default_gov"

	# Restore Max CPU Frequency if its from ECO Mode or using Limit Frequency
	apply_game_freqs

	# VM Cache Pressure
	zeshia "40" "/proc/sys/vm/vfs_cache_pressure"
	zeshia "3" "/proc/sys/vm/drop_caches"

	# Skip If Lite Mode Enabled
	if [ "$(getprop persist.sys.azenithconf.cpulimit)" -eq 0 ]; then
		# Workqueue settings
		zeshia "N" /sys/module/workqueue/parameters/power_efficient
		zeshia "N" /sys/module/workqueue/parameters/disable_numa
		zeshia "0" /sys/kernel/eara_thermal/enable
		zeshia "0" /sys/devices/system/cpu/eas/enable
		zeshia "1" /sys/devices/system/cpu/cpu2/online
		zeshia "1" /sys/devices/system/cpu/cpu3/online
	fi

	if [ "$(/system/bin/getprop persist.sys.azenithconf.dndongaming)" -eq 1 ]; then
		cmd notification set_dnd priority && AZLog "DND disabled"
	fi

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
			if [ -n "$pkg" ] && ! /system/bin/echo "$pkg" | /vendor/bin/grep -qE "com.android.systemui|com.android.settings|$(basename "$0")"; then
				/system/bin/echo "$pkg"
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
	if [ "$(/vendor/bin/getprop persist.sys.azenithconf.memkill)" -eq 1 ]; then
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

	# Skip If Lite Mode Enabled
	if [ "$(getprop persist.sys.azenithconf.cpulimit)" -eq 0 ]; then
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
	fi

	# CPU Power Mode
	zeshia "1" "/proc/cpufreq/cpufreq_cci_mode"
	zeshia "3" "/proc/cpufreq/cpufreq_power_mode"

	# Skip If Lite Mode Enabled
	if [ "$(getprop persist.sys.azenithconf.cpulimit)" -eq 0 ]; then
		# Max GPU Frequency
		if [ -d /proc/gpufreq ]; then
			gpu_freq="$(cat /proc/gpufreq/gpufreq_opp_dump | /vendor/bin/grep -o 'freq = [0-9]*' | sed 's/freq = //' | sort -nr | head -n 1)"
			zeshia "$gpu_freq" /proc/gpufreq/gpufreq_opp_freq
		elif [ -d /proc/gpufreqv2 ]; then
			zeshia 0 /proc/gpufreqv2/fix_target_opp_index
		fi
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

	# Skip If Lite Mode Enabled
	if [ "$(getprop persist.sys.azenithconf.cpulimit)" -eq 0 ]; then
		# UFS Governor's
		zeshia "0" "/sys/devices/platform/10012000.dvfsrc/helio-dvfsrc/dvfsrc_req_ddr_opp"
		zeshia "0" "/sys/kernel/helio-dvfsrc/dvfsrc_force_vcore_dvfs_opp"
		zeshia "performance" "/sys/class/devfreq/mtk-dvfsrc-devfreq/governor"
		zeshia "performance" "/sys/devices/platform/soc/1c00f000.dvfsrc/mtk-dvfsrc-devfreq/devfreq/mtk-dvfsrc-devfreq/governor"
	fi
}

###############################################
# # # # # # # POWERSAVE PROFILE # # # # # # #
###############################################

eco_mode() {
	AZLog "Applying Eco (Powersave) Profile..."

	setgov "powersave" && dlog "Applying governor to : powersave"

	# Power level settings
	for pl in /sys/devices/system/cpu/perf; do
		zeshia 0 "$pl/gpu_pmu_enable"
		zeshia 0 "$pl/fuel_gauge_enable"
		zeshia 0 "$pl/enable"
		zeshia 1 "$pl/charger_enable"
	done

	# Disable DND
	if [ "$(/system/bin/getprop persist.sys.azenithconf.dndongaming)" -eq 1 ]; then
		cmd notification set_dnd off && AZLog "DND disabled"
	fi

	# Set Frequency to lowest
	setsfreqs

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
