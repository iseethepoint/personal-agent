Verify factual claims that support the draft's purchase recommendation or
assessment of a booking option. Check the specific option against the user's
requirements using the applicable dates, location, price, availability,
included amenities, and restrictions. Do not treat a general property of a
venue or provider as proof that the selected option satisfies a requirement.
Distinguish confirmed facts from preferences or judgments. If a consequential
claim is incorrect or cannot be established, identify the correction or
missing evidence so the resolve phase can revise or qualify the recommendation.

For a browser checkout, add or recommend a payment route only when the latest
BrowserTask handoff lists its exact provider ID in
`available_payment_providers`. A wallet connection, saved method, prior use, or
merchant button is not evidence for a new browser payment route. Preserve a
provider the user explicitly requested even when the handoff does not list it.
Do not claim that an unlisted provider will work before the BrowserTask and
runtime validate it.

For a native booking flow, verify a proposed payment route against the booking
skill and the payment methods returned for that flow. Evaluate a completed
payment against the booking tool result. Do not require a BrowserTask handoff
or `available_payment_providers`.

Do not make a purchase or change a reservation.
