# SMDupeRemover
 CLI tool to find and remove duplicate filenames in Soundminer SQLITE databases.  

### USE AT YOUR OWN RISK I OFFER NO SUPPORT OF ANY KIND
### THIS LITERALLY DELETES RECORDS FROM YOUR DATABASE, SO I ADVISE YOU NEVER USE IT UNLESS YOU ARE VERY CERTAIN

Usage: 
    SMDupeRemover <database> arguments

The main program will Copy your database to a new database with _thinned added for identification.  It will then scan the new database for identical filenames and then use logic to decide which one to keep.
The default comparison logic is explained below in configuration.  

You can also COMPARE two databases and remove any file in the main database that also exists in the comparison database.  

You can also optionally have it scan for a series of characters (tags) and remove any files with them.  This is useful for finding all those -PiSH_ and -RVRS_ files protools generates.  There is an included default list, or you can create your own tags.txt.  Again --geneerate-config-files will create these files showing you the default list

The Program runs in the following order if all optional flags are enabled:  
  Compare Database, then Check For Duplicates in the main database, then Prune Tags.


## ARGUMENTS:

### -g or --generate-config-files
generates tags.txt and order.txt. order.txt defines how the main duplicate checker decides which file to keep.  this will overwrite what's there with the default values if they already exist in the directory.  I suggest running once and then modifying from there if you like.  Without these files, the program will just do the default order/tags I have pre programmed in the program.

### -c or --compare <database2>
if any file in the target database exists in the comparison database, it will be removed

### -p or --prune-tags
looks for common Protools Processing Tags and removes files with them.  can use tags.txt to define them.

### -n or --no-filename-check
this doesn't run the normal duplicate filename check on the database.  Useful if you want to just remove tags or compare with another database only.

### -d or --create-duplicates-database
after processing the database it will generate a new database containing all the deletions that were made

### -v or --verbose
displays each file as it's being deleted.  Can FLOOD your terminal

### -y or --no-prompt
automatically responds yes to the processing prompts

### -u or --unsafe
writes DIRECTLY to the database.  Also, skips all the yes, no warnings.  USE WITH CAUTION.

### -h or --help
gives a nice help summary

## CONFIGURATION:
SMDupeRemover has a built in logic and defaults but they can be overridden with the following configuration files

### order.txt
The program has it's own built in logic as far as deciding what logic it will use to chose while file to keep, but if you'd like to adjust it you can.  Just create *order.txt* or you can generate it with --generate-config-files.

The default logic is:  
    duration DESC  
    channels DESC  
    sampleRate DESC  
    bitDepth DESC  
    BWDate ASC  
    scannedDate ASC  

DESC is descending, ASC is ascending. The higher up in the list, the higher the priority, so first it checks duration and works it's way down.  
You can really use any column you like from the Soundminer database and create your own custom order/logic

### tags.txt
When processing audio files in protools, you can get lots of little tags added on to the end of filenames when creating this new media, but ultimately, it's a duplicate of something you already have in your library.  You can use tags.txt to designate what to look for and have removed from your library. You can designate any combination of characters you like.  I've put in a bunch I've found in my library as a default.  I suggest adding away.  I can also add more to the default you think I've missed.  Just send me a message.

tags.txt will only be processed with the --prune-tags option



 
    


