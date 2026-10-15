# Career Mode: economy notes (careerstaff-2)

Reference course: 6 km, 11 stops, all served and punctual, MAXI bus, standard difficulty, about 25 min.
All numbers come from the real `settle`/`simulate` code (scratch test `print_economy`), not from estimates.

## The player at the wheel (unchanged)
| Courses in a row | Total | Per real hour |
|---|---|---|
| 1 | 370 | ~890 |
| 3 | 1 664 | ~1 300 |
| 6 | 4 991 | ~2 000 |
| 9 | 9 984 | ~2 700 |
| 12 | 15 530 | ~3 100 |
| 20 | 30 320 | ~3 600 |

Fares 10.8/km + 21/stop (x3 from careerpay-2), punctuality +25 %, series x1.5 per course up to x5 (careerpay-1).
Costs (fuel 1.55/l, 200 per crash, service 420 per 35 points, inspection 150) are only a few percent of
the takings: the pressure comes from capital (bus 70 000 new for a MAXI, depots 15 000+, places 2 500+).

## Drivers (changed)
- Duration = timetable (first departure -> last arrival of the courses given), real time, 10 h shift at most,
  rest = 1/3 of the shift. Courses chosen on the Jobs page (from the chosen course "until hh:mm").
- No series for drivers (it caused the 40 000 for 28 courses: x4 on average).
- Wage = share of takings: 0.24 + 0.0012 x skill + 0.02 per good trait (26 % at skill 20, 35 % at 90), min 30 per course.
- Company net per hour of a driver (8 courses, standard, mixed weather, bus condition 75):
  skill 20: ~430, skill 50: ~470, skill 90: ~530 (gross 690-840/h, wage 25-33 %, damage + fuel 1-9 %).
- Hire 400 + 35/point (skill 60: 2 500 = about 5 h of its work), retainer 30 + skill/2 per booked session,
  training 250 + 4/point for +3 skill, wear 1 point per 20 km.
- A driver earns roughly 1/2 of what the player earns for a single course and 1/6 of the player's plateau,
  so it is a side income, not a replacement for playing; at most `STAFF_LIMIT` drivers (1, 2, 3, 5, 8 by level) and one bus each.

## Knobs
`company::economy` (fares, FARE_*, KM_PER_POINT, ...), `career_session::STREAK_STEP/STREAK_MAX`,
`staff::{JOB_TIME_SCALE, MAX_SHIFT_SECS, REST_SHARE, MIN_WAGE_PER_COURSE, wage_share, hire_cost, retainer, TRAIN_*}`.

