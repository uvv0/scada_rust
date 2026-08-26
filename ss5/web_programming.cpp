#include "web_programming.h"
#include "module_programming.h"
#include <QFileInfo>

namespace WebProgramming {

quint32 crc32(const QByteArray &data)
{
    quint32 crc=0xFFFFFFFFU;
    for(int i=0;i<data.size();++i){
        crc^=quint8(data.at(i));
        for(int bit=0;bit<8;++bit)
            crc=(crc>>1)^((crc&1U)?0xEDB88320U:0U);
    }
    return crc^0xFFFFFFFFU;
}

quint16 contentTypeForFile(const QString &name)
{
    const QString x=QFileInfo(name).suffix().toLower();
    if(x=="html"||x=="htm")return 1;
    if(x=="css")return 2;
    if(x=="js")return 3;
    if(x=="json")return 4;
    if(x=="png")return 5;
    if(x=="jpg"||x=="jpeg")return 6;
    if(x=="svg")return 7;
    if(x=="ico")return 9;
    return 8;
}

QString defaultPathForFile(const QString &name)
{
    const QString base=QFileInfo(name).fileName();
    return base.compare("index.html",Qt::CaseInsensitive)==0 ?
           QString("/") : QString("/")+base;
}

QByteArray makeBeginConfigFrame(quint16 slot,quint32 total,
    quint16 type,quint32 crc,const QString &path,quint16 station)
{
    QByteArray p=path.toUtf8();
    if(slot>=SlotCount||!total||total>SlotDataSize||p.isEmpty()||
       p.size()>=40||p.at(0)!='/')return QByteArray();
    p.resize(40);
    QVector<quint16> v;
    v<<slot<<quint16(total>>16)<<quint16(total)
     <<0<<0<<0<<type<<quint16(crc>>16)<<quint16(crc);
    for(int i=0;i<40;i+=2)
        v<<quint16((quint16(quint8(p.at(i)))<<8)|quint8(p.at(i+1)));
    return ModuleProgramming::makeWriteMultipleFrame(SlotRegister,v,station);
}

QByteArray makeCommandFrame(quint16 command,quint16 station)
{
    return ModuleProgramming::makeWriteSingleFrame(CommandRegister,command,station);
}

QList<QByteArray> makeChunkDataFrames(const QByteArray &chunk,quint16 station)
{
    QList<QByteArray> frames;
    if(chunk.isEmpty()||chunk.size()>ChunkSize)return frames;
    QVector<quint16> words=ModuleProgramming::imageWords(chunk);
    for(int p=0;p<words.size();){
        int count=qMin(ModuleProgramming::MaxWriteRegisters,words.size()-p);
        frames.append(ModuleProgramming::makeWriteMultipleFrame(
            quint16(ModuleProgramming::DataBase+p),words.mid(p,count),station));
        p+=count;
    }
    return frames;
}

QByteArray makeChunkMetaFrame(quint32 offset,quint16 length,quint16 station)
{
    QVector<quint16> v;
    v<<quint16(offset>>16)<<quint16(offset)<<length;
    return ModuleProgramming::makeWriteMultipleFrame(OffsetHiRegister,v,station);
}

QByteArray makeStatusReadFrame(quint16 station)
{
    return ModuleProgramming::makeReadHoldingFrame(StatusRegister,6,station);
}

bool parseStatus(const QByteArray &frame,Status *out,QString *error)
{
    QVector<quint16> v;
    if(!ModuleProgramming::parseReadHoldingResponse(frame,&v,error))return false;
    if(v.size()!=6){if(error)*error=QString::fromUtf8("Ожидалось 6 слов веб-состояния");return false;}
    if(out){out->status=v[0];out->result=v[1];
        out->written=(quint32(v[2])<<16)|v[3];
        out->crc32=(quint32(v[4])<<16)|v[5];}
    return true;
}

QString statusText(quint16 s)
{
    switch(s){case 0:return QString::fromUtf8("ожидание");
    case 1:return QString::fromUtf8("стирание");case 2:return QString::fromUtf8("готов");
    case 3:return QString::fromUtf8("запись");case 4:return QString::fromUtf8("блок записан");
    case 5:return QString::fromUtf8("завершено");case 6:return QString::fromUtf8("ошибка");
    default:return QString::number(s);}
}

QString resultText(quint16 r)
{
    switch(r){case 0:return QString::fromUtf8("успешно");
    case 0xFFFF:return QString::fromUtf8("неверный слот");
    case 0xFFFE:return QString::fromUtf8("неверный размер");
    case 0xFFFD:return QString::fromUtf8("неверный путь");
    case 0xFFFC:return QString::fromUtf8("ошибка Flash");
    case 0xFFFB:return QString::fromUtf8("ошибка проверки");
    case 0xFFFA:return QString::fromUtf8("неверное состояние");
    default:return QString("0x%1").arg(r,4,16,QChar('0'));}
}
}
