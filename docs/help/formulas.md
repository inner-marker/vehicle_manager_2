# Formulas: calculated fields and derived meters

A **calculated field** shows a value worked out from an event's other values, such as cost per mile on a fuel-up. A **derived meter** is worked out from a vehicle's other meters, for example service miles = odometer + idle hours × 33. Neither is ever typed. They're worked out each time they're shown, and never stored. The **ƒ** badge marks every worked-out value.

Admins and Maintainers add them in the vehicle's **Setup**: **+ Calculated field** under Custom fields, and **+ Derived meter** under Meters.

## Writing a formula

- **Values** by their **key**, such as `cost`, `trip-time`, or `odometer`. Each field's and meter's key is shown in Setup, and the **Values ▾** menu lists them. A key never changes, even when the field or meter is renamed.
- **Numbers**, with a unit if they have one: `33 mi/h`, `1000 mi`, `5 gal`, `90 days`, `50%`.
- **`+ − × ÷`** (or `- * /`), and brackets `( )`. Multiplying and dividing happen before adding and subtracting.
- **Keys can contain hyphens**, so put spaces around a minus: `trip-time - stop-time`, not `trip-time-stop-time` (that reads as one key).

The formula is checked as you type: **✓ Gives money per distance**, or the reason it doesn't work.

## Units

Every value measures something: a **distance**, a **volume**, a **time**, **money**, or nothing (a plain number or a percentage). Units come from the fields and meters themselves, so you never convert miles to meters yourself.

- **`+` and `−`** need the same kind on both sides. `trip-time + volume` is refused: "can't add a time to a volume."
- **`×` and `÷`** combine them: distance ÷ volume is **fuel economy**, money ÷ distance is **money per distance**, and distance ÷ time is a **speed**.
- A calculated field's result must match what it **shows as**. A field that shows money per distance can't hold a formula that gives a volume.
- **Money** keeps its currency. Mixing two currencies gives no value.
- **Percentages:** 80% counts as 0.8. Adding or subtracting percentages gives a percentage (`oil-life − 10%`); multiplying or dividing gives a plain number.

Values show in your units (miles or kilometers, US or imperial gallons or liters, mpg or L/100 km), whatever units they were typed in.

## Values on a calculated field

On an event, a calculated field can use:

- **the event's own fields** (numbers, distances, tread depths (as a distance), times, volumes, fuel economy, and yes/no as 1 or 0), but not pressures or other calculated fields;
- **its meter readings**, derived meters included;
- **on a fuel-up**, `volume`, `cost`, `highway`, `towing`, and `hauling` (percentages), and `reported-economy`;
- **since the last event of the same kind**:
  - `distance-since-last(odometer)`: how far a meter moved since then;
  - `time-since-last`: how long it's been.

  The first event of its kind has no earlier one, so these have no value there.

## Functions

| Function | What it gives |
|---|---|
| `mean(…)` | The average. |
| `median(…)` | The middle value (the average of the middle two, for an even count). |
| `geometric_mean(…)` | The geometric mean. Every value must be above zero. |
| `harmonic_mean(…)` | The harmonic mean, the right average for rates such as mpg. Every value must be above zero. |
| `min(…)`, `max(…)` | The smallest, the largest. |
| `sum(…)` | The total. |
| `count(…)` | How many (a plain number). |
| `stddev(…)` | The sample standard deviation. It needs at least two values. |
| `round(x, places)` | `x` rounded to `places` decimals (0 to 6), **as you see it**, in your units: `round(cost ÷ distance-since-last(odometer), 2)` is to the cent per mile. |
| `abs(x)` | `x` without its sign. |

The statistics functions (all but `round` and `abs`) work in two ways:

- **Over values you list:** `max(trip-time, stop-time)`. They must all measure the same thing.
- **Over past events, with an Over:** one expression, a comma, then which events:

| Over | Which events |
|---|---|
| `last 5` (or `last 5 events`) | The last 5 of this kind, this one included. |
| `last 30 days`, `last 2 weeks` | Those in that time, counting back from this one. |
| `last 3 months` | Those in the last 3 calendar months, in the event's time zone. |
| `last 1000 mi`, `last 1500 km` | Those within that distance on the vehicle's main odometer. |
| `last 1000 mi on trip-odometer` | The same, on another distance meter. |
| `last 50 h on engine-hours` | Those within 50 hours on an hour meter (an hour span always names its meter). |
| `cumulative` | All of them. |

The expression is worked out on each of those events, and events where it has no value are left out. Some examples:

- `mean(trip-time, last 5)`: the average trip time over the last 5 fill-ups.
- `sum(volume, last 3 months)`: the fuel bought in the last 3 months.
- `mean(cost ÷ volume, last 1000 mi)`: the average price per gallon over the last 1,000 miles.

A past-events function can't go inside another one's expression.

## Derived meters

A derived meter uses only:

- the vehicle's **typed meters that aren't reset**;
- **numbers**, **`+ − × ÷`**, and **functions of listed values**.

It can't use `round`, past events, or "since the last". It gets a value at every reading of one of its meters, once each has a reading, from each one's latest reading.

Interval rules and what's due work on a derived meter like any other. It can't be reset or required on events, and nobody types its readings.

Example: `odometer + engine-hours × 33 mi/h` gives service miles.

## When there's no value

A worked-out value shows **—** and the reason when:

- something it uses was left out;
- there's no earlier event for "since the last";
- none of the events in its Over has a value;
- it divides by zero, or mixes currencies;
- a mean needs values above zero, or `stddev` has fewer than two.

## In use

A field or meter that a formula uses can't be removed or turned off, or taken off a kind of event the formula needs. A meter a derived meter uses can't be made resettable. Change or remove the formula first; the message names it.
