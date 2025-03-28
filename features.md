# Screens
- Set View
    - A list of pedalboards that can be played in order
    - Added from individual pedalboards or songs

- Song List

- Song view
    - List of pedalboards in order of a song

- Pedalboard List

- Pedalboard View
    - The pedal chain in the pedalboard
    - Can edit pedalboard
    - Can change name


- Tuner

- Metronome

- Backup Track

# Commands sent to server
setparameter <<pedalboard index>> <<pedal index>> parameter value
move pedalboard <<src index>> <<dest index>>
add pedalboard <<pedalboard stringified>>
delete pedalboard <<pedalboard index>>
add pedal <<pedalboard index>> <<pedal index>> <<pedal stringified>>
delete pedal <<pedalboard index>> <<pedal index>>
move pedal <<pedalboard index>> <<src index>> <<dest index>>
load set <<pedalboardset stringified>>
play <<pedalboard index>>
master <<volume 0-1>>
