for ex in psi pm25 air_temperature relative_humidity wind_speed wind_direction \
  rainfall two_hr_forecast twenty_four_hr_forecast four_day_outlook uv \
  weather_lightning weather_wbgt; do
  echo "=== $ex ==="
  cargo run --quiet --example "$ex" 2>&1 | head -5
  sleep 5
done